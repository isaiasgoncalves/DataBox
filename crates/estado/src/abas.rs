//! As abas de documento abertas e o histórico único de voltar e avançar (aba + endereço), sem egui. A casca
//! guarda um `Abas<Documento>` e desenha só a ativa; os testes usam qualquer `T`.
//!
//! - Sempre há pelo menos uma aba: fechar a última abre o Início, e o Início sozinho não fecha. Por isso
//!   [`Abas::ativa`] nunca falha.
//! - Abrir uma rota cuja [`ChaveAba`] já tem aba foca a que existe, sem trocar o endereço dela (a página, a busca
//!   e os filtros ficam). Só a Consulta SQL não tem chave: `Consulta { sql: Some(..) }` sempre abre outra, e
//!   `Consulta { sql: None }` foca a usada mais recentemente (ou cria uma). As Consultas se numeram pelo menor
//!   número livre (Consulta 1, 2, 3; fechada a 2, a próxima é a 2).
//! - Cada foco novo entra no histórico como [`Entrada`]. Voltar e avançar só focam a aba da entrada: nunca
//!   refazem o documento nem trocam o endereço dele.
//! - [`Abas::substituir`] é o `replaceState` daquela aba: a rota dela muda e, no histórico, também a entrada
//!   atual (se for dela) e as entradas dela que mostravam o endereço de antes. [`Abas::atualizar`] muda a rota e
//!   põe uma entrada nova.
//! - Fechar uma aba poda o histórico (as entradas vizinhas iguais se fundem). Se ela era a ativa, a nova ativa é a
//!   da entrada anterior; sem nenhuma, a vizinha da direita, depois a da esquerda.
//! - [`Abas::reiniciar`] (a troca de banco) fecha todas, inclusive as Consultas, e zera o histórico. Os ids nunca
//!   se repetem. [`Abas::reiniciar_menos`] faz o mesmo mantendo as que a casca escolher (as de arquivo de dados).

use crate::rota::{Navegacao, Parte, Rota};

/// Identidade de uma aba aberta. Não se repete enquanto o app roda, nem depois de [`Abas::reiniciar`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdAba(pub u64);

/// O que faz duas rotas caírem na mesma aba. A Consulta SQL não tem chave: cada uma é uma aba.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ChaveAba {
    Inicio,
    VisaoGeral,
    /// Uma parte de uma tabela ou visão, sem o tipo: o nome é único entre as duas, e a tela de tabela corrige o
    /// tipo por `Acao::Substituir`.
    Parte {
        nome: String,
        parte: Parte,
    },
    /// A ficha de um arquivo de dados, pela chave do caminho (`prefs::chave_caminho`: sem diferença de caixa nem de
    /// barra): o mesmo arquivo, com qualquer grafia, cai na mesma aba.
    Arquivo(String),
}

impl ChaveAba {
    /// A chave da aba desta rota; None na Consulta SQL.
    pub fn de(rota: &Rota) -> Option<ChaveAba> {
        match rota {
            Rota::Inicio => Some(ChaveAba::Inicio),
            Rota::Banco => Some(ChaveAba::VisaoGeral),
            Rota::Tabela { nome, aba, .. } | Rota::Visao { nome, aba, .. } => {
                Some(ChaveAba::Parte { nome: nome.clone(), parte: *aba })
            }
            Rota::Arquivo { caminho } => Some(ChaveAba::Arquivo(prefs::chave_caminho(caminho))),
            Rota::Consulta { .. } => None,
        }
    }
}

/// Uma entrada do histórico: a aba e o endereço que ela mostrava.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entrada {
    pub aba: IdAba,
    pub rota: Rota,
}

/// Uma aba aberta: o endereço e o conteúdo (o documento, na casca). Só o conteúdo muda por fora; o resto muda
/// pelos métodos de [`Abas`].
#[derive(Debug)]
pub struct AbaAberta<T> {
    id: IdAba,
    chave: Option<ChaveAba>,
    rota: Rota,
    numero_consulta: Option<u32>,
    /// Quando ela foi focada pela última vez (o relógio de [`Abas`]).
    usada_em: u64,
    pub conteudo: T,
}

impl<T> AbaAberta<T> {
    pub fn id(&self) -> IdAba {
        self.id
    }

    pub fn chave(&self) -> Option<&ChaveAba> {
        self.chave.as_ref()
    }

    /// O endereço atual da aba (a página, a busca e os filtros da parte Dados; o texto da Consulta).
    pub fn rota(&self) -> &Rota {
        &self.rota
    }

    /// O número da Consulta SQL ("Consulta 2"); None nas outras abas.
    pub fn numero_consulta(&self) -> Option<u32> {
        self.numero_consulta
    }
}

/// O que [`Abas::abrir`] fez.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Abertura {
    /// Criou a aba (o conteúdo nasceu agora).
    Nova(IdAba),
    /// Focou uma aba que já estava aberta.
    Existente(IdAba),
}

impl Abertura {
    pub fn id(self) -> IdAba {
        match self {
            Abertura::Nova(id) | Abertura::Existente(id) => id,
        }
    }
}

/// As abas abertas, na ordem da faixa, com o histórico único. O `criar` dos métodos recebe o id e a rota da
/// aba nova e devolve o conteúdo dela.
#[derive(Debug)]
pub struct Abas<T> {
    lista: Vec<AbaAberta<T>>,
    historico: Navegacao<Entrada>,
    proximo_id: u64,
    relogio: u64,
}

impl<T> Abas<T> {
    /// As abas com uma só, a desta rota.
    pub fn nova(rota: Rota, criar: impl FnOnce(IdAba, &Rota) -> T) -> Abas<T> {
        let mut abas = Abas { lista: Vec::new(), historico: Navegacao::default(), proximo_id: 1, relogio: 0 };
        abas.recomecar(rota, criar);
        abas
    }

    /// Abre a aba desta rota, ou foca a que já tem a mesma chave sem trocar o endereço dela. A Consulta SQL com
    /// um comando sempre abre outra; sem comando, foca a usada mais recentemente (ou cria uma).
    pub fn abrir(&mut self, rota: Rota, criar: impl FnOnce(IdAba, &Rota) -> T) -> Abertura {
        let existente = match &rota {
            Rota::Consulta { sql: Some(_) } => None,
            Rota::Consulta { sql: None } => self.consulta_mais_recente(),
            outra => ChaveAba::de(outra).and_then(|c| self.achar(&c)),
        };
        match existente {
            Some(id) => {
                self.focar(id);
                Abertura::Existente(id)
            }
            None => {
                let id = self.criar_aba(rota, criar);
                self.focar(id);
                Abertura::Nova(id)
            }
        }
    }

    /// Uma Consulta SQL nova, sempre (o "Nova consulta"), com o menor número livre.
    pub fn nova_consulta(&mut self, sql: Option<String>, criar: impl FnOnce(IdAba, &Rota) -> T) -> IdAba {
        let id = self.criar_aba(Rota::Consulta { sql }, criar);
        self.focar(id);
        id
    }

    /// Foca a aba; a entrada dela entra no histórico se ela ainda não era a ativa. False se ela não existe.
    pub fn focar(&mut self, id: IdAba) -> bool {
        if self.id_ativa() != id {
            let Some(rota) = self.get(id).map(|a| a.rota.clone()) else { return false };
            self.historico.ir(Entrada { aba: id, rota });
        }
        self.marcar_uso(id);
        true
    }

    /// Troca o endereço da aba sem entrada nova no histórico (o `replaceState` dela). Se a chave nova é a de outra
    /// aba aberta (uma visão aberta como tabela, que a tela corrigiu), as duas viram uma: esta fecha, a outra fica
    /// com o endereço que já tinha e é focada se esta era a ativa, e a fechada volta para a casca soltar o que ela
    /// fazia.
    pub fn substituir(&mut self, id: IdAba, rota: Rota) -> Option<AbaAberta<T>> {
        self.trocar_rota(id, rota, false)
    }

    /// Troca o endereço da aba e, se ela é a ativa, põe uma entrada nova no histórico, sem refazer nada. A chave
    /// que muda para a de outra aba faz o mesmo que em [`Abas::substituir`].
    pub fn atualizar(&mut self, id: IdAba, rota: Rota) -> Option<AbaAberta<T>> {
        self.trocar_rota(id, rota, true)
    }

    /// Fecha a aba e a devolve (para a casca soltar uma execução em andamento). Fechar a última abre o Início;
    /// o Início, quando é a única aba, não fecha (None).
    pub fn fechar(&mut self, id: IdAba, criar_inicio: impl FnOnce(IdAba, &Rota) -> T) -> Option<AbaAberta<T>> {
        let i = self.posicao(id)?;
        if self.lista.len() > 1 {
            return self.remover(id);
        }
        if self.lista[i].chave == Some(ChaveAba::Inicio) {
            return None;
        }
        let fechada = self.lista.remove(i);
        self.recomecar(Rota::Inicio, criar_inicio);
        Some(fechada)
    }

    /// Fecha as abas que passam em `fechar`, na ordem da faixa, com as mesmas regras de [`Abas::fechar`].
    pub fn fechar_onde(
        &mut self,
        mut fechar: impl FnMut(&AbaAberta<T>) -> bool,
        mut criar_inicio: impl FnMut(IdAba, &Rota) -> T,
    ) -> Vec<AbaAberta<T>> {
        let ids: Vec<IdAba> = self.lista.iter().filter(|a| fechar(a)).map(|a| a.id).collect();
        ids.into_iter().filter_map(|id| self.fechar(id, &mut criar_inicio)).collect()
    }

    /// A troca de banco: fecha todas as abas (devolvidas na ordem da faixa), inclusive as Consultas, abre a desta
    /// rota e zera o histórico.
    pub fn reiniciar(&mut self, rota: Rota, criar: impl FnOnce(IdAba, &Rota) -> T) -> Vec<AbaAberta<T>> {
        let fechadas = std::mem::take(&mut self.lista);
        self.recomecar(rota, criar);
        fechadas
    }

    /// A troca de banco que **mantém** as abas que passam em `manter` (as de arquivo de dados, que não são do
    /// banco): fecha as outras (devolvidas na ordem da faixa), abre a desta rota (ou foca a mantida que já tem a
    /// mesma chave, sem refazê-la nem trocar o endereço dela) e zera o histórico, que passa a ter só a ativa. As
    /// mantidas ficam onde estavam na faixa; a nova entra no fim.
    pub fn reiniciar_menos(
        &mut self,
        rota: Rota,
        criar: impl FnOnce(IdAba, &Rota) -> T,
        manter: impl Fn(&AbaAberta<T>) -> bool,
    ) -> Vec<AbaAberta<T>> {
        let (mantidas, fechadas): (Vec<_>, Vec<_>) = std::mem::take(&mut self.lista).into_iter().partition(&manter);
        self.lista = mantidas;
        match ChaveAba::de(&rota).and_then(|c| self.achar(&c)) {
            Some(id) => {
                let rota = self.lista.iter().find(|a| a.id == id).map(|a| a.rota.clone()).unwrap_or(rota);
                self.historico.reiniciar(Entrada { aba: id, rota });
                self.marcar_uso(id);
            }
            None => self.recomecar(rota, criar),
        }
        fechadas
    }

    pub fn pode_voltar(&self) -> bool {
        self.historico.pode_voltar()
    }

    pub fn pode_avancar(&self) -> bool {
        self.historico.pode_avancar()
    }

    /// Foca a aba da entrada anterior do histórico (sem refazer nada); false se não havia para onde.
    pub fn voltar(&mut self) -> bool {
        let andou = self.historico.voltar();
        if andou {
            self.marcar_uso(self.id_ativa());
        }
        andou
    }

    /// Foca a aba da entrada seguinte do histórico (sem refazer nada); false se não havia para onde.
    pub fn avancar(&mut self) -> bool {
        let andou = self.historico.avancar();
        if andou {
            self.marcar_uso(self.id_ativa());
        }
        andou
    }

    /// A vizinha da ativa na faixa, com volta: a seguinte (Ctrl+Tab) ou a anterior (Ctrl+Shift+Tab). Não muda
    /// nada: quem chama a foca (e o foco entra no histórico). None com uma aba só.
    pub fn vizinha(&self, seguinte: bool) -> Option<IdAba> {
        let n = self.lista.len();
        if n < 2 {
            return None;
        }
        let i = self.indice_ativa();
        let j = if seguinte { (i + 1) % n } else { (i + n - 1) % n };
        self.lista.get(j).map(|a| a.id)
    }

    pub fn id_ativa(&self) -> IdAba {
        self.historico.atual().aba
    }

    pub fn ativa(&self) -> &AbaAberta<T> {
        let i = self.indice_ativa();
        &self.lista[i]
    }

    pub fn ativa_mut(&mut self) -> &mut AbaAberta<T> {
        let i = self.indice_ativa();
        &mut self.lista[i]
    }

    pub fn get(&self, id: IdAba) -> Option<&AbaAberta<T>> {
        self.lista.iter().find(|a| a.id == id)
    }

    pub fn get_mut(&mut self, id: IdAba) -> Option<&mut AbaAberta<T>> {
        self.lista.iter_mut().find(|a| a.id == id)
    }

    /// A aba aberta com esta chave.
    pub fn achar(&self, chave: &ChaveAba) -> Option<IdAba> {
        self.lista.iter().find(|a| a.chave.as_ref() == Some(chave)).map(|a| a.id)
    }

    /// A aba usada (focada) mais recentemente entre as que passam em `filtro` (a ativa, se ela passa).
    pub fn mais_recente_onde(&self, filtro: impl Fn(&AbaAberta<T>) -> bool) -> Option<IdAba> {
        self.lista.iter().filter(|a| filtro(a)).max_by_key(|a| a.usada_em).map(|a| a.id)
    }

    /// As abas na ordem da faixa.
    pub fn iter(&self) -> impl Iterator<Item = &AbaAberta<T>> {
        self.lista.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut AbaAberta<T>> {
        self.lista.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.lista.len()
    }

    /// Sempre false: há pelo menos uma aba.
    pub fn is_empty(&self) -> bool {
        self.lista.is_empty()
    }

    // ------------------------------------------------------------------ por dentro

    /// O índice da ativa na faixa. A entrada atual do histórico é sempre de uma aba aberta e a faixa nunca fica
    /// vazia (nenhum método público a esvazia), então o índice vale sempre.
    fn indice_ativa(&self) -> usize {
        let i = self.posicao(self.id_ativa());
        debug_assert!(i.is_some(), "a entrada atual do histórico é de uma aba aberta");
        i.unwrap_or(0)
    }

    fn posicao(&self, id: IdAba) -> Option<usize> {
        self.lista.iter().position(|a| a.id == id)
    }

    /// Põe a aba no fim da faixa, sem focar.
    fn criar_aba(&mut self, rota: Rota, criar: impl FnOnce(IdAba, &Rota) -> T) -> IdAba {
        let id = IdAba(self.proximo_id);
        self.proximo_id += 1;
        let numero_consulta = matches!(rota, Rota::Consulta { .. }).then(|| self.numero_livre());
        let conteudo = criar(id, &rota);
        self.lista.push(AbaAberta { id, chave: ChaveAba::de(&rota), rota, numero_consulta, usada_em: 0, conteudo });
        id
    }

    /// A única aba passa a ser a desta rota, com o histórico só com ela.
    fn recomecar(&mut self, rota: Rota, criar: impl FnOnce(IdAba, &Rota) -> T) {
        let id = self.criar_aba(rota.clone(), criar);
        self.historico.reiniciar(Entrada { aba: id, rota });
        self.marcar_uso(id);
    }

    /// O menor número de Consulta que nenhuma aba aberta usa.
    fn numero_livre(&self) -> u32 {
        let usado = |n: u32| self.lista.iter().any(|a| a.numero_consulta == Some(n));
        (1..=u32::MAX).find(|&n| !usado(n)).unwrap_or(u32::MAX)
    }

    fn marcar_uso(&mut self, id: IdAba) {
        self.relogio += 1;
        let agora = self.relogio;
        if let Some(a) = self.get_mut(id) {
            a.usada_em = agora;
        }
    }

    fn consulta_mais_recente(&self) -> Option<IdAba> {
        self.mais_recente_onde(|a| matches!(a.rota, Rota::Consulta { .. }))
    }

    fn trocar_rota(&mut self, id: IdAba, rota: Rota, entrada_nova: bool) -> Option<AbaAberta<T>> {
        let i = self.posicao(id)?;
        let chave = ChaveAba::de(&rota);
        if let Some(outra) = chave.as_ref().and_then(|c| self.achar(c)).filter(|&outra| outra != id) {
            if self.id_ativa() == id {
                self.focar(outra);
            }
            return self.remover(id);
        }
        let numero_consulta = match &rota {
            Rota::Consulta { .. } => self.lista[i].numero_consulta.or_else(|| Some(self.numero_livre())),
            _ => None,
        };
        let aba = &mut self.lista[i];
        let antiga = std::mem::replace(&mut aba.rota, rota.clone());
        aba.chave = chave;
        aba.numero_consulta = numero_consulta;
        let ativa = self.id_ativa() == id;
        if entrada_nova && ativa {
            self.historico.ir(Entrada { aba: id, rota });
            return None;
        }
        if ativa {
            self.historico.atual_mut().rota = rota.clone();
        }
        // as entradas que mostravam o endereço de antes desta aba mostram o novo: assim, fechar uma aba que estava
        // entre duas visitas a esta funde as duas entradas, e o voltar não fica parado na mesma aba
        for e in self.historico.entradas_mut().filter(|e| e.aba == id && e.rota == antiga) {
            e.rota = rota.clone();
        }
        None
    }

    /// Tira a aba da faixa e do histórico, quando há outra aberta.
    fn remover(&mut self, id: IdAba) -> Option<AbaAberta<T>> {
        let i = self.posicao(id)?;
        let fechada = self.lista.remove(i);
        if !self.historico.reter(|e| e.aba != id) {
            // nenhuma entrada de antes: a vizinha da direita, depois a da esquerda
            if let Some(vizinha) = self.lista.get(i).or_else(|| self.lista.last()) {
                let entrada = Entrada { aba: vizinha.id, rota: vizinha.rota.clone() };
                self.historico.substituir(entrada);
                // funde com a entrada da frente, se for a mesma
                self.historico.reter(|_| true);
            }
        }
        self.marcar_uso(self.id_ativa());
        Some(fechada)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::rota::{ConsultaDados, LIMITE_HISTORICO};

    /// O conteúdo de cada aba é a rota com que ela nasceu: dá para ver que focar e voltar não refazem nada.
    fn criar(_: IdAba, rota: &Rota) -> Rota {
        rota.clone()
    }

    fn clientes() -> Rota {
        Rota::objeto("clientes", false)
    }

    fn pedidos() -> Rota {
        Rota::objeto("pedidos", false)
    }

    fn na_pagina(rota: &Rota, pagina: u64) -> Rota {
        rota.com_dados(ConsultaDados { pagina, ..Default::default() })
    }

    /// Os endereços das abas na ordem da faixa.
    fn enderecos(abas: &Abas<Rota>) -> Vec<String> {
        abas.iter().map(|a| a.rota().endereco()).collect()
    }

    fn ativa(abas: &Abas<Rota>) -> String {
        abas.ativa().rota().endereco()
    }

    fn consulta_numero(abas: &Abas<Rota>, id: IdAba) -> Option<u32> {
        abas.get(id).and_then(AbaAberta::numero_consulta)
    }

    #[test]
    fn abrir_a_mesma_parte_foca_sem_trocar_o_endereco() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        let Abertura::Nova(id) = abas.abrir(clientes(), criar) else { panic!("devia criar") };
        assert!(abas.substituir(id, na_pagina(&clientes(), 3)).is_none());
        assert!(matches!(abas.abrir(Rota::Banco, criar), Abertura::Existente(_)));
        assert_eq!(ativa(&abas), "#/banco");
        assert_eq!(abas.abrir(clientes(), criar), Abertura::Existente(id));
        assert_eq!(ativa(&abas), "#/t/clientes/dados?pagina=3", "a página fica");
        assert_eq!(abas.ativa().conteudo, clientes(), "o conteúdo não foi refeito");
        // outra parte da mesma tabela é outra aba
        let estrutura = abas.abrir(Rota::objeto_na_aba("clientes", false, Parte::Estrutura), criar);
        assert!(matches!(estrutura, Abertura::Nova(e) if e != id));
        assert_eq!(abas.len(), 3);
    }

    #[test]
    fn visao_e_tabela_com_o_mesmo_nome_sao_a_mesma_chave() {
        let chave = ChaveAba::de(&Rota::objeto("v", true));
        assert_eq!(chave, ChaveAba::de(&Rota::objeto("v", false)));
        assert_eq!(chave, Some(ChaveAba::Parte { nome: "v".into(), parte: Parte::Dados }));
        assert_eq!(ChaveAba::de(&Rota::Inicio), Some(ChaveAba::Inicio));
        assert_eq!(ChaveAba::de(&Rota::Banco), Some(ChaveAba::VisaoGeral));
        assert_eq!(ChaveAba::de(&Rota::Consulta { sql: None }), None);
        let mut abas = Abas::nova(Rota::Banco, criar);
        let a = abas.abrir(Rota::objeto("v", true), criar);
        assert_eq!(abas.abrir(Rota::objeto("v", false), criar), Abertura::Existente(a.id()));
        assert_eq!(abas.achar(&ChaveAba::Parte { nome: "v".into(), parte: Parte::Dados }), Some(a.id()));
    }

    #[test]
    fn consulta_com_sql_sempre_abre_outra() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        let a = abas.abrir(Rota::consulta_com("SELECT 1"), criar);
        let b = abas.abrir(Rota::consulta_com("SELECT 1"), criar);
        assert!(matches!((a, b), (Abertura::Nova(x), Abertura::Nova(y)) if x != y));
        assert_eq!(consulta_numero(&abas, a.id()), Some(1));
        assert_eq!(consulta_numero(&abas, b.id()), Some(2));
        assert_eq!(abas.id_ativa(), b.id());
        assert_eq!(abas.get(abas.id_ativa()).and_then(|a| a.chave()), None);
    }

    #[test]
    fn consulta_sem_sql_foca_a_mais_recente_ou_cria() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        let Abertura::Nova(c1) = abas.abrir(Rota::Consulta { sql: None }, criar) else { panic!("devia criar") };
        abas.abrir(clientes(), criar);
        let c2 = abas.nova_consulta(None, criar);
        assert_eq!(consulta_numero(&abas, c2), Some(2));
        assert!(abas.focar(c1));
        abas.abrir(clientes(), criar);
        assert_eq!(abas.abrir(Rota::Consulta { sql: None }, criar), Abertura::Existente(c1), "a usada por último");
        abas.abrir(Rota::Banco, criar);
        assert!(abas.voltar(), "voltar também conta como uso");
        assert_eq!(abas.id_ativa(), c1);
        assert!(abas.focar(c2));
        abas.abrir(Rota::Banco, criar);
        assert_eq!(abas.abrir(Rota::Consulta { sql: None }, criar), Abertura::Existente(c2));
        // sem nenhuma Consulta aberta, cria
        abas.fechar(c1, criar);
        abas.fechar(c2, criar);
        assert!(matches!(abas.abrir(Rota::Consulta { sql: None }, criar), Abertura::Nova(_)));
        assert_eq!(abas.ativa().numero_consulta(), Some(1));
    }

    #[test]
    fn mais_recente_onde_acha_a_usada_por_ultimo_que_passa() {
        // o "Trazer colunas desta planilha" procura o CSV usado por último
        let mut abas = Abas::nova(Rota::Banco, criar);
        let a = abas.abrir(Rota::Arquivo { caminho: "a.csv".into() }, criar).id();
        let b = abas.abrir(Rota::Arquivo { caminho: "b.csv".into() }, criar).id();
        abas.abrir(clientes(), criar);
        let e_arquivo = |x: &AbaAberta<Rota>| matches!(x.rota(), Rota::Arquivo { .. });
        assert_eq!(abas.mais_recente_onde(e_arquivo), Some(b));
        assert!(abas.focar(a));
        abas.abrir(Rota::Banco, criar);
        assert_eq!(abas.mais_recente_onde(e_arquivo), Some(a));
        assert_eq!(abas.mais_recente_onde(|x| x.numero_consulta().is_some()), None);
    }

    #[test]
    fn consultas_numeradas_pelo_menor_livre() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        let ids: Vec<IdAba> = (0..3).map(|_| abas.nova_consulta(None, criar)).collect();
        let numeros: Vec<Option<u32>> = ids.iter().map(|&id| consulta_numero(&abas, id)).collect();
        assert_eq!(numeros, [Some(1), Some(2), Some(3)]);
        assert!(abas.fechar(ids[1], criar).is_some());
        let nova = abas.nova_consulta(Some("SELECT 2".into()), criar);
        assert_eq!(consulta_numero(&abas, nova), Some(2), "a 2 estava livre");
        let outra = abas.nova_consulta(None, criar);
        assert_eq!(consulta_numero(&abas, outra), Some(4));
        assert_eq!(consulta_numero(&abas, IdAba(1)), None, "a visão geral não tem número");
    }

    #[test]
    fn voltar_e_avancar_atravessam_abas() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        let c = abas.abrir(clientes(), criar).id();
        abas.abrir(pedidos(), criar);
        // a página de clientes muda com a aba escondida (o replaceState dela)
        abas.substituir(c, na_pagina(&clientes(), 3));
        assert!(abas.voltar());
        assert_eq!(abas.id_ativa(), c);
        assert_eq!(ativa(&abas), "#/t/clientes/dados?pagina=3", "voltar não troca o endereço da aba");
        assert!(abas.voltar());
        assert_eq!(ativa(&abas), "#/banco");
        assert!(!abas.voltar() && !abas.pode_voltar());
        assert!(abas.avancar());
        assert!(abas.avancar());
        assert_eq!(ativa(&abas), "#/t/pedidos/dados");
        assert!(!abas.pode_avancar());
        assert_eq!(abas.len(), 3, "nada foi aberto nem refeito");
        assert_eq!(abas.get(c).map(|a| &a.conteudo), Some(&clientes()));
        // ir a outra aba depois de voltar perde o avançar
        assert!(abas.voltar());
        abas.abrir(Rota::Banco, criar);
        assert!(!abas.pode_avancar());
    }

    #[test]
    fn substituir_nao_entra_no_historico_e_vale_para_a_aba_certa() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        let c = abas.abrir(clientes(), criar).id();
        let p = abas.abrir(pedidos(), criar).id();
        assert!(abas.substituir(c, na_pagina(&clientes(), 2)).is_none());
        assert_eq!(abas.id_ativa(), p, "a ativa não muda");
        assert_eq!(abas.get(p).map(|a| a.rota().endereco()).as_deref(), Some("#/t/pedidos/dados"));
        abas.substituir(p, na_pagina(&pedidos(), 5));
        assert_eq!(abas.historico.atual(), &Entrada { aba: p, rota: na_pagina(&pedidos(), 5) });
        // nenhuma entrada nova: o voltar vai direto para clientes, depois para a visão geral
        assert!(abas.voltar());
        assert_eq!(ativa(&abas), "#/t/clientes/dados?pagina=2");
        assert!(abas.voltar());
        assert!(!abas.pode_voltar());
        // um id que não existe não faz nada
        assert!(abas.substituir(IdAba(999), Rota::Banco).is_none());
        assert_eq!(enderecos(&abas), ["#/banco", "#/t/clientes/dados?pagina=2", "#/t/pedidos/dados?pagina=5"]);
    }

    #[test]
    fn substituir_que_muda_a_chave_funde_com_a_existente() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        let v = abas.abrir(Rota::objeto("v", true), criar).id();
        abas.substituir(v, na_pagina(&Rota::objeto("v", true), 2));
        // a visão pedida como tabela, na Estrutura: outra chave, outra aba
        let e = abas.abrir(Rota::objeto_na_aba("v", false, Parte::Estrutura), criar).id();
        assert_ne!(e, v);
        // a tela corrige para a visão na parte Dados, que já tem aba: as duas viram uma
        let fechada = abas.substituir(e, Rota::objeto("v", true));
        assert_eq!(fechada.map(|a| a.id()), Some(e));
        assert_eq!(abas.id_ativa(), v);
        assert_eq!(ativa(&abas), "#/v/v/dados?pagina=2", "a que fica guarda o próprio endereço");
        assert_eq!(abas.len(), 2);
        // as duas visitas à visão se fundiram
        assert!(abas.voltar());
        assert_eq!(ativa(&abas), "#/banco");
        assert!(!abas.pode_voltar());

        // fora da ativa, a fusão não rouba o foco
        let e = abas.abrir(Rota::objeto_na_aba("v", false, Parte::Estrutura), criar).id();
        abas.abrir(Rota::Banco, criar);
        assert!(abas.substituir(e, Rota::objeto("v", true)).is_some());
        assert_eq!(ativa(&abas), "#/banco");
        // o atualizar funde do mesmo jeito
        let e = abas.abrir(Rota::objeto_na_aba("v", false, Parte::Estrutura), criar).id();
        assert!(abas.atualizar(e, Rota::objeto("v", true)).is_some());
        assert_eq!(abas.id_ativa(), v);
    }

    #[test]
    fn atualizar_poe_uma_entrada_nova_na_mesma_aba() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        let c = abas.abrir(clientes(), criar).id();
        assert!(abas.atualizar(c, na_pagina(&clientes(), 2)).is_none());
        assert_eq!(ativa(&abas), "#/t/clientes/dados?pagina=2");
        assert!(abas.voltar());
        assert_eq!(abas.id_ativa(), c, "a entrada de antes é da mesma aba");
        assert_eq!(ativa(&abas), "#/t/clientes/dados?pagina=2", "e voltar a ela só foca a aba");
        assert!(abas.voltar());
        assert_eq!(ativa(&abas), "#/banco");
    }

    #[test]
    fn fechar_a_ativa_vai_para_a_mais_recente_depois_a_vizinha() {
        // a mais recente: a da entrada anterior do histórico
        let mut abas = Abas::nova(Rota::Banco, criar);
        let a = abas.abrir(clientes(), criar).id();
        abas.abrir(pedidos(), criar);
        let c = abas.nova_consulta(None, criar);
        assert!(abas.focar(a));
        assert_eq!(abas.fechar(a, criar).map(|x| x.id()), Some(a));
        assert_eq!(abas.id_ativa(), c);

        // sem entrada anterior: a vizinha da direita
        let mut abas = Abas::nova(Rota::Banco, criar);
        let b = abas.id_ativa();
        let a = abas.abrir(clientes(), criar).id();
        let p = abas.abrir(pedidos(), criar).id();
        assert!(abas.voltar() && abas.voltar());
        assert_eq!(abas.id_ativa(), b);
        abas.fechar(b, criar);
        assert_eq!(abas.id_ativa(), a);
        assert!(!abas.pode_voltar());
        assert!(abas.avancar(), "a entrada da frente fica (e a de clientes se fundiu com a atual)");
        assert_eq!(abas.id_ativa(), p);
        assert!(!abas.pode_avancar());

        // sem entrada anterior nem vizinha à direita: a da esquerda (as entradas mais antigas podem ter saído
        // pelo limite de 100)
        let mut abas = Abas::nova(Rota::Banco, criar);
        let a = abas.abrir(clientes(), criar).id();
        let p = abas.abrir(pedidos(), criar).id();
        abas.historico.reiniciar(Entrada { aba: p, rota: pedidos() });
        assert!(abas.fechar(p, criar).is_some());
        assert_eq!(abas.id_ativa(), a, "a vizinha da esquerda, não a primeira");
        assert_eq!(enderecos(&abas), ["#/banco", "#/t/clientes/dados"]);
        assert!(!abas.pode_voltar() && !abas.pode_avancar());
    }

    #[test]
    fn fechar_poda_o_historico() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        let c = abas.abrir(clientes(), criar).id();
        let p = abas.abrir(pedidos(), criar).id();
        assert!(abas.focar(c));
        // a página muda depois de voltar a clientes: as duas visitas mostram o endereço novo
        abas.substituir(c, na_pagina(&clientes(), 2));
        assert!(abas.fechar(p, criar).is_some());
        assert_eq!(abas.id_ativa(), c);
        assert!(abas.voltar(), "as duas visitas a clientes viraram uma");
        assert_eq!(ativa(&abas), "#/banco");
        assert!(!abas.pode_voltar());
        assert!(abas.avancar());
        assert!(!abas.pode_avancar());
        // fechar_onde fecha as que passam, na ordem da faixa, e devolve todas
        let p = abas.abrir(pedidos(), criar).id();
        let q = abas.nova_consulta(None, criar);
        let ativa_antes = abas.id_ativa();
        let fechadas = abas.fechar_onde(|a| a.id() != ativa_antes && a.chave().is_some(), criar);
        let ids: Vec<IdAba> = fechadas.iter().map(AbaAberta::id).collect();
        assert_eq!(ids, [IdAba(1), c, p]);
        assert_eq!(abas.id_ativa(), q);
        assert_eq!(abas.len(), 1);
        assert!(!abas.pode_voltar());
    }

    #[test]
    fn fechar_a_ultima_abre_o_inicio() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        let b = abas.id_ativa();
        let fechada = abas.fechar(b, criar);
        assert_eq!(fechada.map(|a| a.conteudo), Some(Rota::Banco));
        assert_eq!(abas.len(), 1);
        assert_eq!(abas.ativa().chave(), Some(&ChaveAba::Inicio));
        assert_ne!(abas.id_ativa(), b);
        assert!(!abas.pode_voltar() && !abas.pode_avancar());
        // a Consulta sozinha também fecha e dá lugar ao Início
        let c = abas.nova_consulta(None, criar);
        let inicio = abas.achar(&ChaveAba::Inicio);
        assert!(abas.fechar(inicio.unwrap_or_default(), criar).is_some(), "o Início com outra aba fecha");
        assert!(abas.fechar(c, criar).is_some());
        assert_eq!(enderecos(&abas), ["#/"]);
    }

    #[test]
    fn inicio_sozinho_nao_fecha() {
        let mut abas = Abas::nova(Rota::Inicio, criar);
        let id = abas.id_ativa();
        assert!(abas.fechar(id, criar).is_none());
        assert_eq!(abas.id_ativa(), id);
        assert!(abas.fechar(IdAba(999), criar).is_none());
        assert!(abas.fechar_onde(|_| true, criar).is_empty());
        assert_eq!(abas.len(), 1);
        assert!(!abas.is_empty());
    }

    #[test]
    fn reiniciar_devolve_todas_e_zera_o_historico() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        abas.abrir(clientes(), criar);
        abas.nova_consulta(None, criar);
        abas.nova_consulta(Some("SELECT 1".into()), criar);
        assert!(abas.voltar());
        let fechadas = abas.reiniciar(Rota::Banco, criar);
        let enderecos_fechados: Vec<String> = fechadas.iter().map(|a| a.rota().endereco()).collect();
        assert_eq!(enderecos_fechados, ["#/banco", "#/t/clientes/dados", "#/sql", "#/sql?sql=SELECT 1"]);
        assert_eq!(abas.len(), 1);
        assert!(!abas.pode_voltar() && !abas.pode_avancar());
        assert!(fechadas.iter().all(|a| a.id() < abas.id_ativa()), "os ids não se repetem");
        let c = abas.nova_consulta(None, criar);
        assert_eq!(consulta_numero(&abas, c), Some(1));
    }

    fn arquivo(caminho: &str) -> Rota {
        Rota::arquivo(caminho)
    }

    fn e_de_arquivo(a: &AbaAberta<Rota>) -> bool {
        matches!(a.chave(), Some(ChaveAba::Arquivo(_)))
    }

    #[test]
    fn mesmo_arquivo_com_outra_grafia_cai_na_mesma_aba() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        let a = abas.abrir(arquivo(r"C:\Dados\vendas.csv"), criar);
        assert!(matches!(a, Abertura::Nova(_)));
        // caixa e barra não contam, como em toda chave de caminho
        assert_eq!(abas.abrir(arquivo("c:/dados/VENDAS.csv"), criar), Abertura::Existente(a.id()));
        assert_eq!(abas.ativa().rota(), &arquivo(r"C:\Dados\vendas.csv"), "o endereço da aba que existe fica");
        // outro arquivo é outra aba, e a chave do arquivo não se confunde com a de uma tabela de mesmo nome
        assert!(matches!(abas.abrir(arquivo(r"C:\Dados\outro.csv"), criar), Abertura::Nova(_)));
        assert_eq!(abas.len(), 3);
        assert_eq!(
            abas.achar(&ChaveAba::Arquivo(prefs::chave_caminho(r"C:\DADOS\vendas.CSV"))),
            Some(a.id()),
            "achar pela chave do caminho"
        );
    }

    #[test]
    fn reiniciar_menos_fecha_so_o_que_nao_passa_e_zera_o_historico() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        abas.abrir(clientes(), criar);
        let a = abas.abrir(arquivo(r"C:\p\a.csv"), criar).id();
        abas.nova_consulta(Some("SELECT 1".into()), criar);
        let b = abas.abrir(arquivo(r"C:\p\b.json"), criar).id();
        assert!(abas.voltar(), "há histórico antes");
        let fechadas = abas.reiniciar_menos(Rota::Banco, criar, e_de_arquivo);
        let fechadas: Vec<String> = fechadas.iter().map(|f| f.rota().endereco()).collect();
        assert_eq!(fechadas, ["#/banco", "#/t/clientes/dados", "#/sql?sql=SELECT 1"], "na ordem da faixa");
        // as de arquivo ficam onde estavam, e a Visão geral nova entra no fim, ativa
        assert_eq!(enderecos(&abas), [r"#/arquivo/C:\p\a.csv", r"#/arquivo/C:\p\b.json", "#/banco"]);
        assert_eq!(ativa(&abas), "#/banco");
        assert!(!abas.pode_voltar() && !abas.pode_avancar(), "o histórico recomeça");
        assert!(abas.id_ativa() > b && a < b, "os ids não se repetem");
        // as mantidas continuam como estavam: abrir uma foca, sem refazer
        assert_eq!(abas.abrir(arquivo(r"C:\p\a.csv"), criar), Abertura::Existente(a));
        assert!(abas.pode_voltar());

        // sem nada que passe, é o `reiniciar`
        let fechadas = abas.reiniciar_menos(Rota::Inicio, criar, |_| false);
        assert_eq!(fechadas.len(), 3);
        assert_eq!(enderecos(&abas), ["#/"]);
        assert_eq!(abas.ativa().chave(), Some(&ChaveAba::Inicio));
    }

    #[test]
    fn reiniciar_menos_foca_a_mantida_de_mesma_chave_sem_refazer() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        let a = abas.abrir(arquivo(r"C:\p\a.csv"), criar).id();
        abas.nova_consulta(None, criar);
        let mut criadas = 0;
        let fechadas = abas.reiniciar_menos(
            arquivo("c:/P/A.CSV"),
            |_, rota| {
                criadas += 1;
                rota.clone()
            },
            e_de_arquivo,
        );
        assert_eq!(criadas, 0, "a aba que já existia não é refeita");
        assert_eq!(fechadas.len(), 2);
        assert_eq!(abas.len(), 1);
        assert_eq!(abas.id_ativa(), a);
        assert_eq!(ativa(&abas), r"#/arquivo/C:\p\a.csv", "o endereço dela não troca");
        assert!(!abas.pode_voltar() && !abas.pode_avancar());
    }

    #[test]
    fn vizinha_da_a_volta_sem_mudar_nada() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        assert_eq!((abas.vizinha(true), abas.vizinha(false)), (None, None), "com uma aba só, nada");
        let b = abas.id_ativa();
        let c = abas.abrir(clientes(), criar).id();
        let p = abas.abrir(pedidos(), criar).id();
        assert_eq!(abas.vizinha(true), Some(b), "da última volta à primeira");
        assert_eq!(abas.vizinha(false), Some(c));
        assert_eq!(abas.id_ativa(), p, "só diz qual é");
        // o Ctrl+Tab foca a que ela diz, e o foco entra no histórico
        assert!(abas.focar(abas.vizinha(true).unwrap_or_default()));
        assert_eq!(abas.vizinha(true), Some(c));
        assert_eq!(abas.vizinha(false), Some(p), "da primeira volta à última");
        assert!(abas.voltar());
        assert_eq!(abas.id_ativa(), p);
    }

    #[test]
    fn limite_de_100_entradas() {
        let mut abas = Abas::nova(Rota::Banco, criar);
        let b = abas.id_ativa();
        let c = abas.abrir(clientes(), criar).id();
        for _ in 0..150 {
            abas.focar(b);
            abas.focar(c);
        }
        let mut passos = 0;
        while abas.voltar() {
            passos += 1;
        }
        assert_eq!(passos, LIMITE_HISTORICO);
    }
}
