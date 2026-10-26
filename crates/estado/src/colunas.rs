//! As colunas de todas as tabelas e visões do banco aberto, lidas numa tarefa e guardadas até a estrutura mudar:
//! a doca "Tabelas" da Consulta (Q1) e o autocompletar do editor (Q2) leem daqui.
//!
//! ```ignore
//! // a cada quadro em que as colunas aparecem (lê na primeira vez e depois de cada invalidação):
//! self.colunas.atualizar(ui.ctx(), ctx);
//! if let Some(tabelas) = self.colunas.tabelas() { … }
//! // depois de um comando que mudou a estrutura:
//! self.colunas.invalidar();
//! ```
//!
//! **Quando relê.** A leitura ([`servico::Servico::colunas_do_banco`]) roda numa [`Promessa`], nunca na thread da
//! interface, e só quando alguém pede ([`ColunasDoBanco::atualizar`]). Fica valendo até: o banco aberto mudar (as
//! colunas do outro somem na hora), a versão do estado da casca mudar ([`Contexto::versao`], que sobe quando os
//! objetos são relidos: uma tabela criada, alterada ou excluída em qualquer aba) ou quem a usa chamar
//! [`ColunasDoBanco::invalidar`] (a Consulta, quando um comando dela mudou a estrutura: fora da casca a versão não
//! sobe). Ao reler o mesmo banco, as colunas antigas continuam à vista até as novas chegarem (sem piscar), e uma
//! leitura que já corria quando a estrutura mudou é jogada fora e refeita.

use std::sync::Arc;

use banco::{ColunaResumo, TabelaColunas};
use comum::Resultado;

use crate::contexto::{Contexto, Reacao};
use crate::promessa::Promessa;

/// O cache das colunas do banco aberto (ver o módulo).
#[derive(Default)]
pub struct ColunasDoBanco {
    /// O banco e a versão do estado de onde saiu o que está guardado (ou o que se está lendo).
    banco_id: String,
    versao: u64,
    tabelas: Option<Arc<Vec<TabelaColunas>>>,
    leitura: Option<Promessa<Vec<TabelaColunas>>>,
    /// A estrutura mudou com a leitura correndo: o que ela trouxer já nasce velho.
    obsoleta: bool,
    /// Precisa ler (nunca leu, ou foi invalidado).
    velha: bool,
    erro: Option<String>,
}

impl ColunasDoBanco {
    pub fn novo() -> ColunasDoBanco {
        ColunasDoBanco { velha: true, ..ColunasDoBanco::default() }
    }

    /// Toma a leitura que terminou e começa outra quando precisa (primeira vez, banco trocado, versão do estado
    /// mudada ou [`ColunasDoBanco::invalidar`]). Barato: chame a cada quadro em que as colunas aparecem. Sem banco
    /// aberto, esquece tudo e não lê.
    pub fn atualizar(&mut self, egui: &egui::Context, ctx: &mut Contexto) {
        let banco_id = ctx.banco_id();
        if banco_id != self.banco_id {
            // outro banco: nada do anterior serve, nem a leitura que corria
            *self = ColunasDoBanco { banco_id, versao: ctx.versao, ..ColunasDoBanco::novo() };
        } else if ctx.versao != self.versao {
            self.versao = ctx.versao;
            self.invalidar();
        }
        if let Some(r) = self.leitura.as_mut().and_then(Promessa::tomar) {
            self.leitura = None;
            if std::mem::take(&mut self.obsoleta) {
                self.velha = true;
            } else {
                self.receber(r, ctx);
            }
        }
        if self.velha && self.leitura.is_none() && !self.banco_id.is_empty() {
            self.velha = false;
            self.leitura = Some(ctx.tarefa(egui, |s, id| s.colunas_do_banco(id)));
        }
    }

    fn receber(&mut self, r: Resultado<Vec<TabelaColunas>>, ctx: &mut Contexto) {
        match r {
            Ok(tabelas) => {
                self.tabelas = Some(Arc::new(tabelas));
                self.erro = None;
            }
            Err(e) => {
                if ctx.reagir(&e) == Reacao::Mostrar {
                    self.erro = Some(e.mensagem);
                }
            }
        }
    }

    /// A estrutura mudou: a próxima [`ColunasDoBanco::atualizar`] relê (o que está guardado continua à vista até
    /// lá), e uma leitura em andamento é refeita quando terminar.
    pub fn invalidar(&mut self) {
        self.velha = true;
        if self.leitura.is_some() {
            self.obsoleta = true;
        }
    }

    /// As tabelas (primeiro) e as visões do usuário com as colunas visíveis, cada grupo por nome; None antes da
    /// primeira leitura chegar.
    pub fn tabelas(&self) -> Option<&[TabelaColunas]> {
        self.tabelas.as_deref().map(Vec::as_slice)
    }

    /// As mesmas tabelas numa `Arc`, para levar a uma tarefa ou guardar sem copiar.
    pub fn compartilhadas(&self) -> Option<Arc<Vec<TabelaColunas>>> {
        self.tabelas.clone()
    }

    /// As colunas da tabela ou visão `nome` (o nome exato primeiro, depois sem caixa, como o SQLite); None se ela não
    /// existe ou se as colunas ainda não chegaram.
    pub fn colunas_de(&self, nome: &str) -> Option<&[ColunaResumo]> {
        let tabelas = self.tabelas()?;
        tabelas
            .iter()
            .find(|t| t.nome == nome)
            .or_else(|| tabelas.iter().find(|t| t.nome.eq_ignore_ascii_case(nome)))
            .map(|t| t.colunas.as_slice())
    }

    /// Uma leitura está em andamento (a primeira ou uma releitura).
    pub fn lendo(&self) -> bool {
        self.leitura.is_some()
    }

    /// A frase da última leitura que falhou (some quando uma leitura dá certo).
    pub fn erro(&self) -> Option<&str> {
        self.erro.as_deref()
    }
}

#[cfg(test)]
mod testes {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::apoio_testes::Ambiente;

    /// Chama `atualizar` até a leitura terminar.
    fn esperar(cache: &mut ColunasDoBanco, amb: &Ambiente, versao: u64) {
        let egui = egui::Context::default();
        let inicio = Instant::now();
        loop {
            let mut acoes = Vec::new();
            let mut ctx = Contexto::novo(&amb.servico, &amb.estado, None, &mut acoes).com_casca(versao, false);
            cache.atualizar(&egui, &mut ctx);
            if !cache.lendo() {
                return;
            }
            assert!(inicio.elapsed() < Duration::from_secs(10), "a leitura não terminou");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn executar(amb: &Ambiente, sql: &str) {
        let r = amb.servico.sql(&amb.estado.banco_id(), sql, 10).unwrap();
        assert!(!r.resultados.iter().any(|c| matches!(c, banco::ResultadoComando::Erro { .. })), "{sql}");
    }

    #[test]
    fn le_na_primeira_vez_e_acha_sem_caixa() {
        let amb = Ambiente::exemplo("colunas_le");
        let mut cache = ColunasDoBanco::novo();
        assert!(cache.tabelas().is_none());
        esperar(&mut cache, &amb, 0);
        let tabelas = cache.tabelas().unwrap();
        assert!(tabelas.iter().any(|t| t.nome == "clientes"));
        assert!(cache.colunas_de("CLIENTES").is_some_and(|c| c.iter().any(|c| c.nome == "email")));
        assert!(cache.colunas_de("nao_existe").is_none());
        assert!(cache.erro().is_none());
    }

    #[test]
    fn so_rele_quando_invalidado_ou_com_outra_versao() {
        let amb = Ambiente::exemplo("colunas_rele");
        let mut cache = ColunasDoBanco::novo();
        esperar(&mut cache, &amb, 0);
        executar(&amb, "ALTER TABLE clientes ADD COLUMN apelido TEXT");
        // sem invalidar, fica o que já leu (nenhuma leitura nova)
        esperar(&mut cache, &amb, 0);
        assert!(!cache.colunas_de("clientes").unwrap().iter().any(|c| c.nome == "apelido"));
        // invalidar relê
        cache.invalidar();
        esperar(&mut cache, &amb, 0);
        assert!(cache.colunas_de("clientes").unwrap().iter().any(|c| c.nome == "apelido"));
        // a versão do estado mudada também
        executar(&amb, "CREATE TABLE nova (x INTEGER)");
        esperar(&mut cache, &amb, 1);
        assert!(cache.colunas_de("nova").is_some());
    }

    #[test]
    fn leitura_que_corria_quando_a_estrutura_mudou_e_refeita() {
        let amb = Ambiente::exemplo("colunas_obsoleta");
        let mut cache = ColunasDoBanco::novo();
        let egui = egui::Context::default();
        {
            let mut acoes = Vec::new();
            let mut ctx = Contexto::novo(&amb.servico, &amb.estado, None, &mut acoes);
            cache.atualizar(&egui, &mut ctx);
        }
        assert!(cache.lendo());
        cache.invalidar();
        executar(&amb, "CREATE TABLE depois (y TEXT)");
        esperar(&mut cache, &amb, 0);
        // a primeira leitura (talvez sem a tabela nova) foi refeita
        assert!(cache.colunas_de("depois").is_some());
    }

    #[test]
    fn sem_banco_nao_le() {
        let amb = Ambiente::vazio("colunas_sem_banco");
        let mut cache = ColunasDoBanco::novo();
        esperar(&mut cache, &amb, 0);
        assert!(cache.tabelas().is_none());
        assert!(!cache.lendo());
    }
}
