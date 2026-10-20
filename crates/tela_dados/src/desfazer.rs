//! O Desfazer no banco (prancheta 15, revisão de UI de 19/10/2026): no banco a gravação é imediata, mas a última
//! edição de célula pode ser desfeita. O registro dela ([`Desfazivel`]) fica no contexto do egui, um só para o
//! aplicativo: atravessa a aba refeita (o F5, a releitura depois de incluir ou excluir) e a próxima mudança o
//! troca. O toast da gravação oferece "Desfazer · Ctrl+Z" por [`PRAZO_TOAST`]; o verbo Desfazer (Ctrl+Z, Editar ›
//! Desfazer) vale enquanto não houver outra mudança. Excluir e incluir linhas, mudar a estrutura e uma transação
//! da Consulta SQL que começou ou terminou depois desligam o Desfazer, com o porquê na dica.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

use banco::TabelaInfo;
use comum::{Celula, Chave};
use egui::Id;

use crate::regras::titulo_linha;

/// Quanto tempo o toast da gravação oferece o Desfazer.
pub const PRAZO_TOAST: f64 = 6.0;
/// A carga da ação do toast do Desfazer começa com isto (a casca devolve ao documento as cargas assim).
pub const PREFIXO_ACAO_TOAST: &str = "tela_dados:";
/// O rótulo do botão do toast da gravação.
pub const ROTULO_ACAO: &str = "Desfazer · Ctrl+Z";

pub const DICA_EXCLUIR: &str = "Desfazer (excluir linhas não se desfaz)";
pub const DICA_INCLUIR: &str = "Desfazer (incluir linhas não se desfaz)";
pub const DICA_ESTRUTURA: &str = "Desfazer (mudar a estrutura não se desfaz)";
pub const DICA_TRANSACAO: &str = "Desfazer (a transação da Consulta SQL mudou depois da edição)";

/// A última edição de célula gravada no banco.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UltimaEdicao {
    /// Identidade do registro (a carga da ação do toast o leva: um toast velho não desfaz a edição de agora).
    pub serial: u64,
    pub banco_id: String,
    pub tabela: String,
    /// A estrutura da tabela na hora da edição ([`estrutura_de`]).
    pub estrutura: u64,
    /// A edição entrou numa transação aberta na Consulta SQL.
    pub transacao: bool,
    /// A chave da linha depois da gravação (a PK editada muda a chave).
    pub chave: Chave,
    pub coluna: String,
    /// O valor de antes, que o Desfazer grava de volta.
    pub antigo: Celula,
    /// O valor relido depois da gravação: o Desfazer só grava se a célula ainda o tem.
    pub gravado: Celula,
}

/// O que o Desfazer da aba Dados tem à mão.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Desfazivel {
    Edicao(UltimaEdicao),
    /// A última mudança não se desfaz (excluir, incluir): o Desfazer fica desligado com esta dica.
    Bloqueado {
        banco_id: String,
        tabela: String,
        dica: &'static str,
    },
}

fn id_registro() -> Id {
    Id::new("tela_dados::desfazer")
}

/// O registro de agora.
pub(crate) fn ler(ectx: &egui::Context) -> Option<Desfazivel> {
    ectx.data(|d| d.get_temp::<Desfazivel>(id_registro()))
}

/// Troca o registro (None: nada a desfazer).
pub(crate) fn guardar(ectx: &egui::Context, registro: Option<Desfazivel>) {
    ectx.data_mut(|d| {
        if let Some(r) = registro {
            d.insert_temp(id_registro(), r);
        } else {
            d.remove::<Desfazivel>(id_registro());
        }
    });
}

/// Um serial novo para um registro.
pub(crate) fn proximo_serial() -> u64 {
    static SERIAL: AtomicU64 = AtomicU64::new(1);
    SERIAL.fetch_add(1, Ordering::Relaxed)
}

/// A carga da ação do toast deste registro.
pub(crate) fn carga(serial: u64) -> String {
    format!("{PREFIXO_ACAO_TOAST}desfazer:{serial}")
}

/// O serial de uma carga da ação do toast do Desfazer (None: a carga é de outro).
pub(crate) fn serial_da_carga(carga: &str) -> Option<u64> {
    carga.strip_prefix(PREFIXO_ACAO_TOAST)?.strip_prefix("desfazer:")?.parse().ok()
}

/// Uma impressão da estrutura da tabela (o CREATE, as colunas, os índices e os gatilhos; não as linhas): muda
/// quando a estrutura muda, e a tela relida dela desliga o Desfazer.
pub(crate) fn estrutura_de(info: &TabelaInfo) -> u64 {
    let mut h = DefaultHasher::new();
    format!("{:?}{:?}{:?}{:?}{:?}", info.sql, info.chave, info.colunas, info.indices, info.gatilhos).hash(&mut h);
    h.finish()
}

/// "linha 3", "linha P003", "linha de pedido_id 7 · produto P003".
fn linha_na_frase(chave: &Chave) -> String {
    let titulo = titulo_linha(Some(chave), 0);
    match titulo.strip_prefix('L') {
        Some(resto) => format!("l{resto}"),
        None => titulo,
    }
}

/// O toast da gravação: "Gravado em clientes (linha 3, email)".
pub fn frase_gravado(tabela: &str, chave: &Chave, coluna: &str) -> String {
    format!("Gravado em {tabela} ({}, {coluna})", linha_na_frase(chave))
}

/// O toast do Desfazer: "Edição desfeita em clientes (linha 3, email)".
pub fn frase_desfeito(tabela: &str, chave: &Chave, coluna: &str) -> String {
    format!("Edição desfeita em {tabela} ({}, {coluna})", linha_na_frase(chave))
}

/// O que a aba diz do Desfazer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Situacao {
    /// Nada a desfazer nesta tabela: desligado, com a dica padrão.
    Nada,
    /// Desligado, com o porquê.
    Desligado(&'static str),
    /// Ligado, com a dica que diz o quê ("Desfazer email da linha 3 (Ctrl+Z)").
    Ligado(String),
}

/// O Desfazer de uma aba (`tabela`, com a estrutura `estrutura`) no banco `banco_id`, com ou sem transação aberta
/// agora.
pub(crate) fn situacao(
    registro: Option<&Desfazivel>,
    banco_id: &str,
    tabela: &str,
    estrutura: u64,
    transacao_aberta: bool,
) -> Situacao {
    match registro {
        Some(Desfazivel::Bloqueado { banco_id: b, tabela: t, dica }) if b == banco_id && t == tabela => {
            Situacao::Desligado(dica)
        }
        Some(Desfazivel::Edicao(u)) if u.banco_id == banco_id && u.tabela == tabela => {
            if u.estrutura != estrutura {
                Situacao::Desligado(DICA_ESTRUTURA)
            } else if u.transacao != transacao_aberta {
                Situacao::Desligado(DICA_TRANSACAO)
            } else {
                Situacao::Ligado(format!("Desfazer {} da {} (Ctrl+Z)", u.coluna, linha_na_frase(&u.chave)))
            }
        }
        _ => Situacao::Nada,
    }
}

#[cfg(test)]
mod testes {
    use comum::Valor;

    use super::*;

    fn edicao(transacao: bool) -> Desfazivel {
        Desfazivel::Edicao(UltimaEdicao {
            serial: 7,
            banco_id: "b1".into(),
            tabela: "clientes".into(),
            estrutura: 42,
            transacao,
            chave: Chave::Rowid(3),
            coluna: "email".into(),
            antigo: Celula::Texto("a@x".into()),
            gravado: Celula::Texto("b@x".into()),
        })
    }

    #[test]
    fn frases_do_toast() {
        assert_eq!(frase_gravado("clientes", &Chave::Rowid(3), "email"), "Gravado em clientes (linha 3, email)");
        let pk = Chave::Pk(vec![("codigo".into(), Valor::Texto("P003".into()))]);
        assert_eq!(frase_gravado("produtos", &pk, "preco"), "Gravado em produtos (linha P003, preco)");
        let composta =
            Chave::Pk(vec![("pedido_id".into(), Valor::Inteiro(7)), ("produto".into(), Valor::Texto("P003".into()))]);
        assert_eq!(
            frase_desfeito("itens_pedido", &composta, "qtd"),
            "Edição desfeita em itens_pedido (linha de pedido_id 7 · produto P003, qtd)"
        );
    }

    #[test]
    fn carga_leva_o_serial() {
        assert_eq!(serial_da_carga(&carga(12)), Some(12));
        assert_eq!(serial_da_carga("tela_csv:marcar_sem_par:1"), None);
        assert_eq!(serial_da_carga("tela_dados:outra:1"), None);
        assert!(proximo_serial() < proximo_serial());
    }

    #[test]
    fn situacao_do_desfazer() {
        let e = edicao(false);
        assert_eq!(
            situacao(Some(&e), "b1", "clientes", 42, false),
            Situacao::Ligado("Desfazer email da linha 3 (Ctrl+Z)".into())
        );
        // outra tabela, outro banco, nada guardado: nada a desfazer aqui
        assert_eq!(situacao(Some(&e), "b1", "pedidos", 42, false), Situacao::Nada);
        assert_eq!(situacao(Some(&e), "b2", "clientes", 42, false), Situacao::Nada);
        assert_eq!(situacao(None, "b1", "clientes", 42, false), Situacao::Nada);
        // a estrutura mudou, a transação começou ou terminou
        assert_eq!(situacao(Some(&e), "b1", "clientes", 43, false), Situacao::Desligado(DICA_ESTRUTURA));
        assert_eq!(situacao(Some(&e), "b1", "clientes", 42, true), Situacao::Desligado(DICA_TRANSACAO));
        let na_transacao = edicao(true);
        assert!(matches!(situacao(Some(&na_transacao), "b1", "clientes", 42, true), Situacao::Ligado(_)));
        assert_eq!(situacao(Some(&na_transacao), "b1", "clientes", 42, false), Situacao::Desligado(DICA_TRANSACAO));
        // excluir linhas não se desfaz
        let b = Desfazivel::Bloqueado { banco_id: "b1".into(), tabela: "clientes".into(), dica: DICA_EXCLUIR };
        assert_eq!(situacao(Some(&b), "b1", "clientes", 42, false), Situacao::Desligado(DICA_EXCLUIR));
        assert_eq!(situacao(Some(&b), "b1", "pedidos", 42, false), Situacao::Nada);
    }
}
