//! A barra de status (o `footer.vidro` das pranchetas Main, Dados, Consulta e DadosClaro), sobre a peça
//! `interface::pecas::barra_status`. Desde a revisão de 20/10/2026 ela fica escondida por padrão (Exibir › Barra de
//! status a mostra): o que ela diz já está nas Propriedades (o modo de gravação na linha "Gravação" da seção Arquivo,
//! [`linha_da_gravacao`]; o SQLite e a codificação), nos rodapés dos documentos e nas abas.
//!
//! - à esquerda, o banco aberto com o ponto (`--ok`; `--aviso` com a transação aberta) e o modo de gravação
//!   ("Gravação imediata", "Transação aberta" em `--aviso` 600 ou "Só leitura"), com a frase que antes era a nota da
//!   lateral na dica; sem banco e com uma pasta aberta, o nome dela com o ponto `--t3` e o caminho na dica; sem nada,
//!   "Nenhuma pasta aberta" com o ponto `--t3` (decisão 17 da etapa 4, prancheta 4). O modo só aparece com banco;
//! - à direita, o que a aba à vista diz de si ("clientes · 60 linhas · 7 colunas", "SQL"), a versão do SQLite e a
//!   codificação. O tamanho do arquivo não vai mais na Visão geral (D1, opção B, da revisão de 15/10/2026): ele já
//!   está nos cartões e na seção Arquivo das Propriedades.
//!
//! Numa aba de arquivo que diz o próprio status (`estado::Documento::status_do_arquivo`, os editores da etapa 5), a
//! barra é a do arquivo (pranchetas 7 e 8): à esquerda o ponto (`--acct` alterado, `--ok` salvo, `--perigo` com
//! erro), o nome e "Alterado, não salvo" ou "Salvo"; à direita os detalhes ("CSV · UTF-8 · CRLF · Separador ;").
//!
//! Os textos que dependem do estado ([`TextosStatus`]) são refeitos com os outros textos da casca, quando o estado
//! muda.

use banco::BancoInfo;
use egui::Ui;
use estado::{Estado, Situacao, StatusDeArquivo};
use interface::pecas::barra_status::{ItemStatus, barra_status};
use interface::tema::paleta;

use super::{frase_da_transacao, nome_da_consulta};

/// O modo de gravação do banco aberto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Modo {
    GravacaoImediata,
    TransacaoAberta,
    SoLeitura,
}

impl Modo {
    /// O texto curto da barra (e o rótulo para o leitor de tela).
    pub(crate) fn rotulo(self) -> &'static str {
        match self {
            Modo::GravacaoImediata => "Gravação imediata",
            Modo::TransacaoAberta => "Transação aberta",
            Modo::SoLeitura => "Só leitura",
        }
    }
}

/// O modo do banco e a frase da dica (as frases da nota da lateral de antes). O só leitura vem antes da transação. Na
/// transação, a dica é o texto do aviso do topo do documento ([`frase_da_transacao`]), com o nome da Consulta que a
/// abriu (`consulta`, de [`nome_da_consulta`]).
pub(crate) fn modo_do_banco(b: &BancoInfo, consulta: &str) -> (Modo, String) {
    let nome = &b.nome;
    if b.somente_leitura {
        // só leitura sem a pessoa pedir (arquivo protegido): o serviço diz o motivo
        let motivo = b.motivo_somente_leitura.as_deref().unwrap_or("Nada é gravado no arquivo.");
        (Modo::SoLeitura, format!("{nome} está aberto só para leitura. {motivo}"))
    } else if b.transacao_aberta {
        (Modo::TransacaoAberta, frase_da_transacao(consulta))
    } else {
        (Modo::GravacaoImediata, format!("Você está editando {nome}. Tudo o que for salvo grava no arquivo na hora."))
    }
}

/// A linha "Gravação" da seção Arquivo das Propriedades do banco (revisão de 20/10/2026: com a barra de status
/// escondida por padrão, o modo de gravação mora aqui): "Imediata", ou "Transação aberta" e "Só leitura" na pílula de
/// espera, com a frase da barra na dica. A transação, com o nome genérico da Consulta ([`nome_da_consulta`]).
pub(crate) fn linha_da_gravacao(d: &mut interface::pecas::detalhes::Detalhes<'_>, b: &BancoInfo) {
    use interface::pecas::rotulos::{TipoPill, pill};
    let (modo, frase) = modo_do_banco(b, &nome_da_consulta(None));
    d.linha("Gravação", |ui| {
        let resp = match modo {
            Modo::GravacaoImediata => pill(ui, "imediata", TipoPill::Neutra),
            Modo::TransacaoAberta => pill(ui, "transação aberta", TipoPill::Wait),
            Modo::SoLeitura => pill(ui, "só leitura", TipoPill::Wait),
        };
        resp.on_hover_text(frase);
    });
}

/// A barra sem banco e sem pasta (prancheta 4).
pub(crate) const SEM_NADA: &str = "Nenhuma pasta aberta";

/// Os textos da barra que dependem do estado.
pub(crate) struct TextosStatus {
    /// O nome do banco; sem ele, o da pasta aberta; sem nada, [`SEM_NADA`].
    banco: String,
    /// A dica do primeiro item: o caminho da pasta, quando é ela que aparece.
    dica: Option<String>,
    tem_banco: bool,
    transacao: bool,
    modo: Option<(Modo, String)>,
    /// "SQLite 3.53.2".
    sqlite: String,
    codificacao: Option<String>,
}

impl TextosStatus {
    pub(crate) fn de(estado: &Estado) -> TextosStatus {
        let b = estado.banco.as_ref();
        let pasta = estado.pasta.as_ref().filter(|_| b.is_none());
        TextosStatus {
            banco: match (b, pasta) {
                (Some(b), _) => b.nome.clone(),
                (None, Some(p)) => p.nome.clone(),
                (None, None) => SEM_NADA.to_string(),
            },
            dica: pasta.map(|p| p.caminho.display().to_string()),
            tem_banco: b.is_some(),
            transacao: b.is_some_and(|b| b.transacao_aberta),
            modo: b.map(|b| modo_do_banco(b, &nome_da_consulta(None))),
            sqlite: format!("SQLite {}", estado.app.sqlite_versao),
            codificacao: b.map(|b| b.codificacao.clone()).filter(|c| !c.is_empty()),
        }
    }
}

/// A cor do ponto da barra de um arquivo (prancheta 7 e 8): `--acct` alterado, `--ok` salvo, `--perigo` com erro e
/// `--t3` só leitura.
pub(crate) fn cor_da_situacao(e: &interface::tema::Estudio, situacao: Situacao) -> egui::Color32 {
    match situacao {
        Situacao::Salvo => e.ok,
        Situacao::Alterado => e.acct,
        Situacao::ComErro => e.perigo,
        Situacao::SoLeitura => e.t3,
    }
}

/// A barra de uma aba de arquivo (o `footer.vidro` das pranchetas 7 e 8): à esquerda o ponto, o nome e a situação
/// ("Alterado, não salvo", "Salvo"); à direita os detalhes, um por item ("CSV", "UTF-8", "CRLF", "Separador ;"),
/// sem o banco, o SQLite nem a codificação dele.
fn barra_do_arquivo(ui: &mut Ui, arquivo: &StatusDeArquivo) {
    let e = paleta(ui).estudio;
    let esquerda = [
        ItemStatus::new(&arquivo.nome).ponto(cor_da_situacao(&e, arquivo.situacao)),
        ItemStatus::new(arquivo.rotulo_da_situacao()),
    ];
    let direita: Vec<ItemStatus<'_>> = arquivo.detalhes.iter().map(|d| ItemStatus::new(d)).collect();
    barra_status(ui, &esquerda, &direita);
}

/// Desenha a barra na largura de `ui`. `documento` é o que a aba à vista diz de si; `consulta` é o nome da Consulta
/// que abriu a transação (a dica do modo cita ela, e a aba pode fechar ou mudar de número entre os quadros dos
/// textos guardados, por isso a frase da transação é refeita aqui). `arquivo` é o que a aba de arquivo diz de si
/// (`estado::Documento::status_do_arquivo`): com ele, a barra é a do arquivo ([`barra_do_arquivo`]), no lugar da do
/// banco.
pub(crate) fn barra(
    ui: &mut Ui,
    t: &TextosStatus,
    documento: Option<&str>,
    consulta: &str,
    arquivo: Option<&StatusDeArquivo>,
) {
    if let Some(a) = arquivo {
        barra_do_arquivo(ui, a);
        return;
    }
    let e = paleta(ui).estudio;
    let ponto = match (t.tem_banco, t.transacao) {
        (false, _) => e.t3,
        (true, true) => e.aviso,
        (true, false) => e.ok,
    };
    let mut primeiro = ItemStatus::new(&t.banco).ponto(ponto);
    if let Some(d) = &t.dica {
        primeiro = primeiro.dica(d);
    }
    let mut esquerda = vec![primeiro];
    let da_transacao;
    if let Some((modo, frase)) = &t.modo {
        let frase = if *modo == Modo::TransacaoAberta {
            da_transacao = frase_da_transacao(consulta);
            &da_transacao
        } else {
            frase
        };
        let mut item = ItemStatus::new(modo.rotulo()).dica(frase);
        if *modo == Modo::TransacaoAberta {
            item = item.cor(e.aviso).forte();
        }
        esquerda.push(item);
    }
    let mut direita: Vec<ItemStatus<'_>> = documento.map(ItemStatus::new).into_iter().collect();
    direita.push(ItemStatus::new(&t.sqlite));
    if let Some(c) = &t.codificacao {
        direita.push(ItemStatus::new(c));
    }
    barra_status(ui, &esquerda, &direita);
}

#[cfg(test)]
mod testes {
    use super::*;
    use estado::apoio_testes::Ambiente;

    #[test]
    fn as_tres_frases_do_modo() {
        let amb = Ambiente::exemplo("status_frases");
        let mut b = amb.estado.banco.clone().expect("banco");
        b.nome = "vendas.db".into();
        assert_eq!(
            modo_do_banco(&b, "Consulta SQL"),
            (
                Modo::GravacaoImediata,
                "Você está editando vendas.db. Tudo o que for salvo grava no arquivo na hora.".to_string()
            )
        );
        b.transacao_aberta = true;
        assert_eq!(
            modo_do_banco(&b, "Consulta 1"),
            (
                Modo::TransacaoAberta,
                "Há uma transação aberta na Consulta 1. O que for salvo agora só vai para o arquivo depois do COMMIT; \
                 fechar o banco descarta tudo desde o BEGIN."
                    .to_string()
            )
        );
        // o só leitura vem antes da transação; sem o motivo, a frase de sempre
        b.somente_leitura = true;
        assert_eq!(
            modo_do_banco(&b, "Consulta SQL"),
            (Modo::SoLeitura, "vendas.db está aberto só para leitura. Nada é gravado no arquivo.".to_string())
        );
        b.motivo_somente_leitura = Some("O arquivo está protegido contra gravação.".into());
        assert_eq!(
            modo_do_banco(&b, "Consulta SQL").1,
            "vendas.db está aberto só para leitura. O arquivo está protegido contra gravação."
        );
        assert_eq!(
            [Modo::GravacaoImediata, Modo::TransacaoAberta, Modo::SoLeitura].map(Modo::rotulo),
            ["Gravação imediata", "Transação aberta", "Só leitura"]
        );
    }

    #[test]
    fn textos_com_e_sem_banco() {
        let amb = Ambiente::exemplo("status_textos");
        let t = TextosStatus::de(&amb.estado);
        assert_eq!(t.banco, "exemplo.db");
        assert!(t.tem_banco && !t.transacao);
        assert_eq!(t.modo.as_ref().map(|m| m.0), Some(Modo::GravacaoImediata));
        assert!(t.sqlite.starts_with("SQLite 3."), "{}", t.sqlite);
        assert_eq!(t.codificacao.as_deref(), Some("UTF-8"));
        let mut sem_banco = amb.estado.clone();
        sem_banco.banco = None;
        let t = TextosStatus::de(&sem_banco);
        assert_eq!(t.banco, "Nenhuma pasta aberta");
        assert!(!t.tem_banco && t.modo.is_none() && t.codificacao.is_none() && t.dica.is_none());
        // com a pasta e sem banco, o nome dela e o caminho na dica; com banco, o banco
        let mut com_pasta = sem_banco.clone();
        let projeto = estado::apoio_testes::abs_texto("Dados/Projeto");
        com_pasta.pasta = Some(estado::PastaAberta::nova(&projeto));
        let t = TextosStatus::de(&com_pasta);
        assert_eq!((t.banco.as_str(), t.dica.as_deref()), ("Projeto", Some(projeto.as_str())));
        assert!(!t.tem_banco && t.modo.is_none());
        let mut os_dois = amb.estado.clone();
        os_dois.pasta = com_pasta.pasta.clone();
        let t = TextosStatus::de(&os_dois);
        assert_eq!((t.banco.as_str(), t.dica), ("exemplo.db", None));
        assert!(t.modo.is_some());
    }
}
