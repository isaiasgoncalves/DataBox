//! Cada operação do serviço, com os argumentos tipados. Os diálogos nativos ficam na interface (rfd).

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use banco::{
    AcaoManutencao, Direcao, FiltroExportacao, GatilhoResumo, Operador, PedidoPagina, ResultadoComando, TipoChave,
    TipoObjeto,
};
use comum::{Celula, Chave, Codigo, Entrada, ErroBanco, Valor};
use ddl::ColunaDef;
use exportar::{Formato, Opcoes};
use prefs::{Aparencia, CoresCodigo, Mudanca, MudancaAparencia, MudancaExportacao, OpcoesExportacao, Tema};

use super::leitura::{Json, ler_csv, ler_csv_texto, ler_json, ler_xlsx, ler_zip, sem_bom};
use super::*;
use crate::{BlobSalvo, ColunaIndicePedida, OperacaoEsquema, OrigemExportacao, Servico};

fn vals(pares: &[(&str, Entrada)]) -> Vec<(String, Entrada)> {
    pares.iter().map(|(c, e)| (c.to_string(), e.clone())).collect()
}

fn tx(s: &str) -> Entrada {
    Entrada::Texto(s.to_string())
}

fn ct(s: &str) -> Celula {
    Celula::Texto(s.to_string())
}

fn pk(codigo: &str) -> Chave {
    Chave::Pk(vec![("codigo".to_string(), Valor::Texto(codigo.to_string()))])
}

fn col(nome: &str, tipo: &str) -> ColunaDef {
    ColunaDef { nome: nome.into(), tipo: tipo.into(), ..ColunaDef::default() }
}

fn tabela(nome: &str) -> OrigemExportacao {
    OrigemExportacao::Tabela { nome: nome.into(), filtro: FiltroExportacao::default() }
}

fn consulta(sql: &str) -> OrigemExportacao {
    OrigemExportacao::Consulta { sql: sql.into() }
}

fn banco_com(tabelas: &[&str]) -> OrigemExportacao {
    OrigemExportacao::Banco { tabelas: Some(tabelas.iter().map(|t| t.to_string()).collect()) }
}

fn extensao(f: Formato) -> &'static str {
    f.como_str()
}

fn colunas(a: &Ambiente, tabela: &str) -> Vec<String> {
    a.servico.tabela(&a.id(), tabela).unwrap().info.colunas.into_iter().map(|c| c.nome).collect()
}

fn linhas_de(r: &ResultadoComando) -> &Vec<Vec<Celula>> {
    match r {
        ResultadoComando::Linhas { linhas, .. } => linhas,
        outro => panic!("esperava linhas, veio {outro:?}"),
    }
}

/// Linhas de dados por nome (planilha, tabela do JSON, CSV do .zip ou "dados").
fn ler_exportado(caminho: &Path, formato: Formato) -> BTreeMap<String, usize> {
    match formato {
        Formato::Xlsx => ler_xlsx(caminho).into_iter().map(|f| (f.nome.clone(), f.linhas() - 1)).collect(),
        Formato::Json => match ler_json(caminho) {
            Json::Lista(itens) => BTreeMap::from([("dados".to_string(), itens.len())]),
            dados => {
                let Json::Objeto(tabelas) = dados.campo("tabelas") else { panic!("sem tabelas") };
                tabelas.iter().map(|(k, v)| (k.clone(), v.lista().len())).collect()
            }
        },
        Formato::Csv if caminho.extension().is_some_and(|e| e == "zip") => ler_zip(caminho)
            .into_iter()
            .map(|(n, d)| (n.trim_end_matches(".csv").to_string(), ler_csv_texto(&sem_bom(&d), ';').len() - 1))
            .collect(),
        Formato::Csv => BTreeMap::from([("dados".to_string(), ler_csv(caminho, ';').len() - 1)]),
    }
}

// ---- aplicação e preferências ------------------------------------------------------------------------

#[test]
fn estado() {
    let a = ambiente("estado");
    let r = a.servico.estado().unwrap();
    assert_eq!(r.app.nome, "DataBox");
    assert_eq!(r.app.versao, env!("CARGO_PKG_VERSION"));
    assert_eq!(r.app.sqlite_versao, rusqlite::version());
    let banco = r.banco.unwrap();
    assert_eq!(banco.nome, "exemplo.db");
    assert_eq!(banco.caminho, canonico(&a.exemplo));
    assert_eq!(r.prefs.recentes[0].nome, "exemplo.db");
}

#[test]
fn estado_sem_banco() {
    let pasta = Pasta::nova("sem-banco");
    let s = Servico::novo(prefs_em(&pasta));
    assert!(s.estado().unwrap().banco.is_none());
    assert_eq!(erro_com(s.info_banco(""), 409).mensagem, "Nenhum banco aberto.");
    erro_com(s.objetos(""), 409);
    erro_com(s.tabela("", "clientes"), 409);
    erro_com(s.historico(""), 409);
}

#[test]
fn banco_inicial_que_nao_abre_levanta_erro() {
    // sem banco_inicial no serviço: o editor abre o banco da linha de comando por abrir_banco
    let pasta = Pasta::nova("inicial");
    let s = Servico::novo(prefs_em(&pasta));
    erro_com(s.abrir_banco(&pasta.join("nao_existe.db"), false, false), 404);
    let lixo = pasta.join("lixo.db");
    fs::write(&lixo, b"isto nao e um banco sqlite".repeat(100)).unwrap();
    erro(s.abrir_banco(&lixo, false, false));
    assert!(s.estado().unwrap().banco.is_none());
}

#[test]
fn prefs_tema_e_exportacao() {
    let a = ambiente("prefs");
    assert_eq!(a.servico.gravar_prefs(Some(Tema::Escuro), None).unwrap().tema, Tema::Escuro);
    let virgula = MudancaExportacao { csv_separador: Some(','), ..MudancaExportacao::default() };
    let r = a.servico.gravar_prefs(None, Some(virgula)).unwrap();
    assert_eq!(r.exportacao.csv_separador, ',');
    assert_eq!(r.tema, Tema::Escuro);
    erro_com(Tema::de_texto("roxo"), 400);
    let decimal = MudancaExportacao { csv_decimal: Some(';'), ..MudancaExportacao::default() };
    erro_com(a.servico.gravar_prefs(None, Some(decimal)), 400);
}

#[test]
fn gravar_preferencias_devolve_o_resumo_com_a_aparencia() {
    let a = ambiente("preferencias");
    assert_eq!(a.servico.estado().unwrap().prefs.aparencia, Aparencia::default());
    let mudanca = Mudanca {
        tema: Some(Tema::Claro),
        aparencia: Some(MudancaAparencia {
            cores_codigo: Some(CoresCodigo::SolarizedEscuro),
            fonte_codigo: Some(Some("Courier New".to_string())),
            tamanho_codigo: Some(14.0),
        }),
        exportacao: Some(MudancaExportacao { json_indentado: Some(false), ..MudancaExportacao::default() }),
        ..Mudanca::default()
    };
    let r = a.servico.gravar_preferencias(mudanca).unwrap();
    let esperada = Aparencia {
        cores_codigo: CoresCodigo::SolarizedEscuro,
        fonte_codigo: Some("Courier New".to_string()),
        tamanho_codigo: 14.0,
    };
    assert_eq!(r.aparencia, esperada);
    assert_eq!(r.tema, Tema::Claro);
    assert!(!r.exportacao.json_indentado);
    // o mesmo que o estado e as preferências dizem depois
    let estado = a.servico.estado().unwrap().prefs;
    assert_eq!((estado.tema, estado.exportacao, estado.aparencia), (r.tema, r.exportacao, r.aparencia.clone()));
    assert_eq!(a.servico.prefs().aparencia(), esperada);
    // o gravar_prefs de antes não mexe na aparência
    assert_eq!(a.servico.gravar_prefs(Some(Tema::Escuro), None).unwrap().aparencia, esperada);
    // tamanho fora da faixa: 400, nada muda
    let ruim = Mudanca::aparencia(MudancaAparencia { tamanho_codigo: Some(30.0), ..MudancaAparencia::default() });
    erro_com(a.servico.gravar_preferencias(ruim), 400);
    assert_eq!(a.servico.prefs().aparencia(), esperada);
}

#[test]
fn remover_recente() {
    let a = ambiente("recente");
    assert!(a.servico.remover_recente(&a.exemplo).unwrap().recentes.is_empty());
}

#[test]
fn lembrar_diretorio_guarda_a_pasta_do_arquivo_escolhido() {
    // o que o /api/dialogo/abrir fazia depois do diálogo
    let a = ambiente("diretorio");
    a.servico.lembrar_diretorio(&a.exemplo);
    assert_eq!(a.servico.prefs().ultimo_diretorio().map(PathBuf::from), Some(a.pasta.caminho.clone()));
}

// ---- arquivos ----------------------------------------------------------------------------------------

#[test]
fn revelar_e_abrir_arquivo() {
    let a = ambiente("revelar");
    let com_aspas = PathBuf::from(format!("\"{}\"", a.exemplo.display()));
    let revelar = a.servico.preparar_revelar(&com_aspas).unwrap();
    assert_eq!(revelar.argumentos.to_string_lossy(), format!("/select,\"{}\"", a.exemplo.display()));
    let abrir = a.servico.preparar_abrir(&a.exemplo).unwrap();
    assert_eq!(abrir.argumentos.to_string_lossy(), format!("\"{}\"", a.exemplo.display()));
    // recusados antes de qualquer programa ser iniciado
    erro_com(a.servico.revelar(Path::new("  ")), 400);
    erro_com(a.servico.abrir_arquivo(Path::new("")), 400);
}

#[test]
fn revelar_arquivo_inexistente_da_404() {
    let a = ambiente("revelar-404");
    erro_com(a.servico.revelar(&a.join("sumiu.xlsx")), 404);
    erro_com(a.servico.abrir_arquivo(&a.join("sumiu.xlsx")), 404);
}

// ---- banco -------------------------------------------------------------------------------------------

#[test]
fn abrir_outro_banco_fecha_o_anterior() {
    let a = ambiente("abrir-outro");
    let anterior = a.servico.atual().unwrap();
    let vazio = amostras::criar_banco_vazio(a.join("vazio.db")).unwrap();
    let info = a.servico.abrir_banco(&vazio, false, false).unwrap();
    assert_eq!(info.nome, "vazio.db");
    assert!(!info.somente_leitura);
    assert_eq!(info.contagem.tabelas, 0);
    assert!(!Arc::ptr_eq(&a.servico.atual().unwrap(), &anterior));
    assert_eq!(erro(anterior.objetos()).status, 409); // a conexão do anterior foi fechada
    assert_eq!(a.servico.prefs().recentes()[0].nome, "vazio.db");
    assert_eq!(a.servico.info_banco(&a.id()).unwrap().nome, "vazio.db");
}

#[test]
fn abrir_banco_que_falha_mantem_o_atual() {
    let a = ambiente("abrir-falha");
    let atual = a.servico.atual().unwrap();
    let e = erro_com(a.servico.abrir_banco(&a.join("nao_existe.db"), false, false), 404);
    assert!(e.mensagem.contains("não existe"), "{e:?}");
    let lixo = a.join("lixo.db");
    fs::write(&lixo, b"isto nao e um banco sqlite".repeat(100)).unwrap();
    erro_com(a.servico.abrir_banco(&lixo, false, false), 400);
    assert!(Arc::ptr_eq(&a.servico.atual().unwrap(), &atual));
    assert_eq!(a.servico.info_banco(&a.id()).unwrap().nome, "exemplo.db");
}

#[test]
fn abrir_banco_somente_leitura_e_com_aspas() {
    let a = ambiente("aspas");
    let vazio = amostras::criar_banco_vazio(a.join("vazio.db")).unwrap();
    let r = a.servico.abrir_banco(Path::new(&format!("  \"{}\" ", vazio.display())), true, false).unwrap();
    assert!(r.somente_leitura);
    assert_eq!(r.caminho, canonico(&vazio));
}

#[test]
fn abrir_banco_pedido_invalido() {
    let a = ambiente("pedido-invalido");
    assert_eq!(
        erro_com(a.servico.abrir_banco(Path::new(""), false, false), 400).mensagem,
        "Informe o caminho do banco."
    );
    erro_com(a.servico.abrir_banco(Path::new(" \"\" "), false, false), 400);
}

#[test]
fn novo_banco() {
    let a = ambiente("novo");
    let destino = a.join("novo.db");
    let r = a.servico.novo_banco(&destino, false).unwrap();
    assert_eq!(r.nome, "novo.db");
    assert_eq!(r.contagem.tabelas, 0);
    assert!(destino.exists());
    assert_eq!(a.servico.prefs().recentes()[0].nome, "novo.db");
    let e = erro_com(a.servico.novo_banco(&a.exemplo, false), 409);
    assert!(e.mensagem.contains("não está vazio"), "{e:?}");
    erro_com(a.servico.novo_banco(&a.join("nao").join("existe.db"), false), 404);
    erro_com(a.servico.novo_banco(&a.pasta.caminho, false), 400);
    assert_eq!(a.servico.info_banco(&a.id()).unwrap().nome, "novo.db");
}

#[test]
fn novo_banco_sobre_arquivo_vazio() {
    let a = ambiente("novo-vazio");
    let vazio = a.join("zero.db");
    fs::write(&vazio, b"").unwrap();
    a.servico.novo_banco(&vazio, false).unwrap();
}

#[test]
fn fechar_banco() {
    let a = ambiente("fechar");
    let anterior = a.servico.atual().unwrap();
    let id = a.id();
    a.servico.fechar_banco(&id, false).unwrap();
    assert!(a.servico.atual().is_none());
    assert_eq!(erro(anterior.objetos()).status, 409);
    assert!(a.servico.estado().unwrap().banco.is_none());
    erro_com(a.servico.info_banco(""), 409);
    // sem banco: quem não mostra banco nenhum fecha sem erro; quem ainda mostra o que fechou é avisado
    a.servico.fechar_banco("", false).unwrap();
    let e = erro_com(a.servico.fechar_banco(&id, false), 409);
    assert!(e.tem_codigo(Codigo::BancoTrocado) && e.mensagem == crate::MSG_BANCO_TROCADO, "{e:?}");
}

#[test]
fn objetos() {
    let a = ambiente("objetos");
    let r = a.servico.objetos(&a.id()).unwrap();
    let nomes: HashSet<&str> = r.tabelas.iter().map(|t| t.nome.as_str()).collect();
    assert_eq!(nomes, HashSet::from(TABELAS_EXEMPLO));
    assert_eq!(r.visoes.iter().map(|v| v.nome.as_str()).collect::<Vec<_>>(), ["v_pedidos_por_cliente"]);
    let clientes = r.tabelas.iter().find(|t| t.nome == "clientes").unwrap();
    assert_eq!(clientes.linhas, Some(N_CLIENTES));
    assert!(r.indices.iter().any(|i| i.nome == "idx_pedidos_cliente"));
    assert_eq!(r.gatilhos, [GatilhoResumo { nome: "trg_pedido_cancelado".into(), tabela: "pedidos".into() }]);
}

#[test]
fn info_banco() {
    let a = ambiente("info");
    let r = a.servico.info_banco(&a.id()).unwrap();
    assert_eq!(r.nome, "exemplo.db");
    assert!(r.chaves_estrangeiras);
    assert_eq!(r.contagem.visoes, 1);
}

#[test]
fn manutencao() {
    let a = ambiente("manutencao");
    let r = a.servico.manutencao(&a.id(), AcaoManutencao::Integridade).unwrap();
    assert_eq!(r.acao.como_str(), "integridade");
    assert!(r.ok);
    assert!(!r.mensagens.is_empty());
    assert!(r.tamanho_antes > 0 && r.tamanho_depois > 0);
    assert!(a.servico.manutencao(&a.id(), AcaoManutencao::Chaves).unwrap().ok);
    assert!(a.servico.manutencao(&a.id(), AcaoManutencao::Vacuum).unwrap().ok);
    erro_com(AcaoManutencao::de_texto("formatar"), 400);
}

#[test]
fn manutencao_que_grava_em_somente_leitura() {
    let a = ambiente_leitura("manutencao-ro");
    erro_com(a.servico.manutencao(&a.id(), AcaoManutencao::Vacuum), 403);
    assert!(a.servico.manutencao(&a.id(), AcaoManutencao::Integridade).unwrap().ok);
}

#[test]
fn copia_de_seguranca() {
    let a = ambiente("copia");
    let destino = a.join("copia.db");
    let r = a.servico.copia(&a.id(), &destino, false).unwrap();
    assert_eq!(r.caminho, destino);
    assert!(r.tamanho > 0);
    assert_eq!(cru(&destino, "SELECT count(*) FROM clientes"), [[Valor::Inteiro(amostras::N_CLIENTES)]]);
    erro_com(a.servico.copia(&a.id(), &a.exemplo, false), 400);
    erro_com(a.servico.copia(&a.id(), &a.join("nao").join("c.db"), false), 404);
}

// ---- tabela e dados ----------------------------------------------------------------------------------

#[test]
fn tabela_com_definicoes() {
    let a = ambiente("tabela");
    let t = a.servico.tabela(&a.id(), "clientes").unwrap();
    assert_eq!(t.info.nome, "clientes");
    assert_eq!(t.info.tipo, TipoObjeto::Table);
    assert!(t.info.editavel);
    assert!(t.info.esquema_editavel);
    assert_eq!(t.info.chave, Some(TipoChave::Rowid { alias: "rowid".into() }));
    let definicoes = t.definicoes.unwrap();
    let nomes: Vec<&str> = definicoes.iter().map(|d| d.nome.as_str()).collect();
    assert_eq!(nomes, ["id", "nome", "email", "cidade", "limite", "ativo", "criado_em"]);
    assert!(definicoes[0].pk);
    assert!(definicoes[0].autoincremento);
    assert_eq!(t.restricoes, Some(vec![]));
    assert_eq!(t.info.linhas, Some(N_CLIENTES));
    assert_eq!(t.aviso_esquema, None);
}

#[test]
fn tabela_com_restricao_de_tabela() {
    let a = ambiente("restricao");
    let itens = a.servico.tabela(&a.id(), "itens_pedido").unwrap();
    assert!(itens.restricoes.unwrap().iter().any(|r| r.contains("PRIMARY KEY")));
    let produtos = a.servico.tabela(&a.id(), "produtos").unwrap();
    assert!(produtos.info.sem_rowid);
    assert_eq!(produtos.info.chave, Some(TipoChave::Pk { colunas: vec!["codigo".into()] }));
}

#[test]
fn tabela_sem_definicoes() {
    let a = ambiente("sem-definicoes");
    for (nome, tipo) in [
        ("v_pedidos_por_cliente", TipoObjeto::View),
        ("notas", TipoObjeto::Virtual),
        ("sqlite_sequence", TipoObjeto::Table),
    ] {
        let t = a.servico.tabela(&a.id(), nome).unwrap();
        assert_eq!(t.info.tipo, tipo, "{nome}");
        assert_eq!(t.definicoes, None, "{nome}");
        assert_eq!(t.restricoes, None, "{nome}");
        assert!(!t.info.esquema_editavel, "{nome}");
        assert_eq!(t.aviso_esquema, None, "{nome}");
    }
}

#[test]
fn tabela_com_definicao_que_nao_analisa() {
    let a = ambiente("nao-analisa");
    fazer_definicoes_falharem(Some(ErroBanco::novo("A definição usa uma sintaxe que o editor não conhece.")));
    let t = a.servico.tabela(&a.id(), "eventos");
    fazer_definicoes_falharem(None);
    let t = t.unwrap();
    assert_eq!(t.definicoes, None);
    assert_eq!(t.restricoes, None);
    assert!(!t.info.esquema_editavel);
    assert!(t.aviso_esquema.unwrap().contains("sintaxe que o editor não conhece"));
    assert!(t.info.editavel);
}

#[test]
fn tabela_inexistente_e_sem_nome() {
    let a = ambiente("inexistente");
    assert!(erro_com(a.servico.tabela(&a.id(), "fantasma"), 404).mensagem.contains("não existe"));
}

#[test]
fn tabela_com_nome_estranho() {
    let a = ambiente("nome-estranho");
    assert_eq!(colunas(&a, "Tabela com espaço"), ["coluna \"citada\"", "número"]);
}

#[test]
fn contagem() {
    let a = ambiente("contagem");
    let c = a.servico.contagem(&a.id(), "eventos", None, &[], None).unwrap();
    assert_eq!(c.linhas, N_EVENTOS);
    assert!(c.tempo_ms >= 0.0);
    erro_com(a.servico.contagem(&a.id(), "fantasma", None, &[], None), 404);
}

#[test]
fn linhas_paginadas_ordenadas_e_filtradas() {
    let a = ambiente("linhas");
    let id = a.id();
    let r = a.servico.linhas(&id, "clientes", &PedidoPagina { pagina: 2, por_pagina: 10, ..PedidoPagina::default() });
    let r = r.unwrap();
    assert_eq!(r.total, Some(N_CLIENTES));
    assert_eq!(r.paginas, Some(6));
    assert_eq!(r.pagina, 2);
    assert_eq!(r.linhas[0].chave, Some(Chave::Rowid(11)));
    assert_eq!(r.colunas[0], "id");

    let pedido =
        PedidoPagina { ordem: Some("id".into()), direcao: Direcao::Desc, por_pagina: 5, ..PedidoPagina::default() };
    let r = a.servico.linhas(&id, "pedidos", &pedido).unwrap();
    let ids: Vec<Celula> = r.linhas.iter().map(|l| l.valores[0].clone()).collect();
    assert_eq!(ids, (0..5).map(|i| Celula::Inteiro(amostras::N_PEDIDOS - i)).collect::<Vec<_>>());
    assert_eq!(r.ordem.as_deref(), Some("id"));
    assert_eq!(r.direcao, Direcao::Desc);

    let filtros = vec![banco::Filtro::texto("status", Operador::Igual, "cancelado")];
    let r = a.servico.linhas(&id, "pedidos", &PedidoPagina { filtros, ..PedidoPagina::default() }).unwrap();
    assert!(r.total.unwrap() > 0);
    let indice = r.colunas.iter().position(|c| c == "status").unwrap();
    assert!(r.linhas.iter().all(|l| l.valores[indice] == ct("cancelado")));

    let busca = PedidoPagina { busca: Some("@example.test".into()), ..PedidoPagina::default() };
    assert_eq!(a.servico.linhas(&id, "clientes", &busca).unwrap().total, Some(N_CLIENTES));
}

#[test]
fn linhas_com_parametros_invalidos() {
    let a = ambiente("linhas-invalidas");
    let pedido = PedidoPagina { ordem: Some("nao_existe".into()), ..PedidoPagina::default() };
    erro_com(a.servico.linhas(&a.id(), "clientes", &pedido), 400);
    erro_com(Direcao::de_texto("lado"), 400);
}

#[test]
fn linha_por_rowid_e_por_pk() {
    let a = ambiente("linha");
    let r = a.servico.linha(&a.id(), "pedidos", &Chave::Rowid(10)).unwrap();
    assert_eq!(r.chave, Some(Chave::Rowid(10)));
    match r.valor("observacao") {
        Some(Celula::Texto(t)) => assert!(t.chars().count() > 300), // texto longo vem inteiro na janela da linha
        outro => panic!("{outro:?}"),
    }
    let r = a.servico.linha(&a.id(), "produtos", &pk("P004")).unwrap();
    assert_eq!(r.valor("serie"), Some(&Celula::Inteiro(9_007_199_254_740_993 + 4)));
    erro_com(a.servico.linha(&a.id(), "pedidos", &Chave::Rowid(99999)), 404);
}

#[test]
fn inserir_atualizar_excluir() {
    let a = ambiente("gravar");
    let id = a.id();
    let valores = vals(&[
        ("nome", tx("Quitéria Figueira")),
        ("email", tx("quiteria.teste@example.test")),
        ("limite", tx("1500,50")),
        ("cidade", Entrada::Padrao),
    ]);
    let nova = a.servico.inserir(&id, "clientes", &valores).unwrap();
    assert!(!nova.transacao_aberta);
    let chave = nova.resultado.chave.clone().unwrap();
    assert_eq!(chave, Chave::Rowid(amostras::N_CLIENTES + 1));
    let linha = nova.resultado.linha.unwrap();
    assert_eq!(linha[1], ct("Quitéria Figueira"));
    assert_eq!(linha[3], ct("São Paulo"));
    assert_eq!(linha[4], Celula::Real(1500.5));

    let r = a.servico.atualizar(&id, "clientes", &chave, &vals(&[("cidade", tx("Recife"))])).unwrap();
    assert_eq!(r.resultado.linha.unwrap()[3], ct("Recife"));

    let r = a.servico.excluir(&id, "clientes", &[chave]).unwrap();
    assert_eq!((r.resultado, r.transacao_aberta), (1, false));
    assert_eq!(a.servico.contagem(&id, "clientes", None, &[], None).unwrap().linhas, N_CLIENTES);
}

#[test]
fn gravacao_com_erro_de_regra() {
    let a = ambiente("regra");
    let id = a.id();
    let e = erro_com(a.servico.inserir(&id, "clientes", &vals(&[("nome", Entrada::Nulo)])), 400);
    assert!(e.mensagem.contains("nulo"), "{e:?}");
    let r = a.servico.sql(&id, "SELECT email FROM clientes WHERE id = 1", 1000).unwrap();
    let Celula::Texto(email) = linhas_de(&r.resultados[0])[0][0].clone() else { panic!("sem e-mail") };
    let e = erro_com(a.servico.inserir(&id, "clientes", &vals(&[("nome", tx("Teste")), ("email", tx(&email))])), 400);
    assert!(e.mensagem.contains("repetição") || e.mensagem.contains("Já existe"), "{e:?}");
    erro_com(a.servico.atualizar(&id, "clientes", &Chave::Rowid(1), &vals(&[("nome", Entrada::Padrao)])), 400);
    erro_com(a.servico.inserir(&id, "v_pedidos_por_cliente", &vals(&[("nome", tx("x"))])), 403);
}

#[test]
fn excluir_com_chave_composta_de_tabela_sem_rowid() {
    let a = ambiente("excluir-pk");
    // P010 é referenciado por itens_pedido (chave estrangeira sem ON DELETE): ou sai, ou a regra impede
    match a.servico.excluir(&a.id(), "produtos", &[pk("P010")]) {
        Ok(r) => assert_eq!(r.resultado, 1),
        Err(e) => {
            assert_eq!(e.status, 400);
            assert!(e.mensagem.contains("chave estrangeira"), "{e:?}");
        }
    }
}

#[test]
fn somente_leitura_recusa_gravacao() {
    let a = ambiente_leitura("so-leitura");
    let id = a.id();
    let t = a.servico.tabela(&id, "clientes").unwrap();
    assert!(!t.info.editavel);
    assert!(!t.info.esquema_editavel);
    erro_com(a.servico.inserir(&id, "clientes", &vals(&[("nome", tx("X"))])), 403);
    erro_com(a.servico.atualizar(&id, "clientes", &Chave::Rowid(1), &vals(&[("nome", tx("X"))])), 403);
    erro_com(a.servico.excluir(&id, "clientes", &[Chave::Rowid(1)]), 403);
    let r = a.servico.sql(&id, "DELETE FROM clientes", 1000).unwrap();
    assert_eq!(r.resultados[0].tipo(), "erro");
}

#[test]
fn salvar_blob() {
    let a = ambiente("blob");
    let id = a.id();
    let destino = a.join("foto.bin");
    let r = a.servico.salvar_blob(&id, "produtos", &pk("P003"), "foto", &destino, false).unwrap();
    assert_eq!(r, BlobSalvo { caminho: destino.clone(), bytes: 64 + 3 * 16 });
    assert_eq!(fs::metadata(&destino).unwrap().len(), 64 + 3 * 16);
    a.pasta.sem_temporarios();
    let e = erro_com(a.servico.salvar_blob(&id, "produtos", &pk("P001"), "foto", &a.join("vazio.bin"), false), 400);
    assert!(e.mensagem.contains("NULL"), "{e:?}");
    let longe = a.join("nao").join("existe.bin");
    erro_com(a.servico.salvar_blob(&id, "produtos", &pk("P003"), "foto", &longe, false), 404);
    erro_com(a.servico.salvar_blob(&id, "produtos", &pk("P003"), "nao_existe", &destino, false), 404);
}

// ---- estrutura ---------------------------------------------------------------------------------------

fn adicionar_origem(padrao: Option<&str>) -> OperacaoEsquema {
    let mut coluna = col("origem", "TEXT");
    coluna.padrao = padrao.map(str::to_string);
    OperacaoEsquema::AdicionarColuna { tabela: "eventos".into(), coluna }
}

#[test]
fn esquema_previa_nao_muda_nada() {
    let a = ambiente("previa");
    let antes = colunas(&a, "eventos");
    let r = a.servico.esquema(&a.id(), &adicionar_origem(None), true).unwrap();
    assert!(r.previa);
    assert!(!r.reconstroi);
    assert!(r.comandos.iter().any(|c| c.contains("ADD COLUMN")));
    assert!(r.sql.ends_with(';'));
    assert!(!r.transacao_aberta);
    assert_eq!(colunas(&a, "eventos"), antes);
}

#[test]
fn esquema_executa() {
    let a = ambiente("executa");
    let r = a.servico.esquema(&a.id(), &adicionar_origem(Some("'teste'")), false).unwrap();
    assert!(!r.previa);
    assert_eq!(colunas(&a, "eventos").last().map(String::as_str), Some("origem"));
}

#[test]
fn esquema_reconstrucao_e_criacao() {
    let a = ambiente("reconstrucao");
    let id = a.id();
    let mut nova = col("observacao", "TEXT");
    nova.padrao = Some("''".into());
    let alterar = OperacaoEsquema::AlterarColuna { tabela: "pedidos".into(), coluna: "observacao".into(), nova };
    let previa = a.servico.esquema(&id, &alterar, true).unwrap();
    assert!(previa.reconstroi);
    assert!(!previa.avisos.is_empty());

    let mut chave = col("id", "INTEGER");
    chave.pk = true;
    let mut razao = col("razao_social", "TEXT");
    razao.nao_nulo = true;
    let criar = OperacaoEsquema::CriarTabela {
        nome: "fornecedores".into(),
        colunas: vec![chave, razao],
        restricoes: vec![],
        sem_rowid: false,
    };
    a.servico.esquema(&id, &criar, false).unwrap();
    assert!(a.servico.objetos(&id).unwrap().tabelas.iter().any(|t| t.nome == "fornecedores"));

    let indice = OperacaoEsquema::CriarIndice {
        nome: "idx_forn_razao".into(),
        tabela: "fornecedores".into(),
        colunas: vec![ColunaIndicePedida { nome: "razao_social".into(), desc: false }],
        unico: true,
        onde: None,
    };
    a.servico.esquema(&id, &indice, false).unwrap();
    let indices = a.servico.tabela(&id, "fornecedores").unwrap().info.indices;
    assert!(indices.iter().any(|i| i.nome == "idx_forn_razao" && i.unico));

    a.servico.esquema(&id, &OperacaoEsquema::ExcluirIndice { nome: "idx_forn_razao".into() }, false).unwrap();
    let renomear = OperacaoEsquema::RenomearTabela { tabela: "fornecedores".into(), novo_nome: "parceiros".into() };
    a.servico.esquema(&id, &renomear, false).unwrap();
    a.servico.esquema(&id, &OperacaoEsquema::ExcluirTabela { tabela: "parceiros".into() }, false).unwrap();
    let nomes: HashSet<String> = a.servico.objetos(&id).unwrap().tabelas.into_iter().map(|t| t.nome).collect();
    assert_eq!(nomes, TABELAS_EXEMPLO.iter().map(|t| t.to_string()).collect());
}

#[test]
fn esquema_visao_e_gatilho() {
    let a = ambiente("visao");
    let id = a.id();
    let criar = OperacaoEsquema::CriarVisao {
        nome: "v_ativos".into(),
        sql: "SELECT id, nome FROM clientes WHERE ativo = 1".into(),
    };
    a.servico.esquema(&id, &criar, false).unwrap();
    assert!(a.servico.objetos(&id).unwrap().visoes.iter().any(|v| v.nome == "v_ativos"));
    a.servico.esquema(&id, &OperacaoEsquema::ExcluirVisao { nome: "v_ativos".into() }, false).unwrap();
    let r = a.servico.esquema(&id, &OperacaoEsquema::ExcluirGatilho { nome: "trg_pedido_cancelado".into() }, true);
    assert!(r.unwrap().sql.contains("DROP TRIGGER"));
    // criar_gatilho também é uma operação (a interface não usa)
    let gatilho = OperacaoEsquema::CriarGatilho {
        sql: "CREATE TRIGGER trg_teste AFTER INSERT ON eventos BEGIN SELECT 1; END".into(),
    };
    assert!(a.servico.esquema(&id, &gatilho, true).unwrap().sql.contains("CREATE TRIGGER"));
}

#[test]
fn esquema_pedidos_invalidos() {
    let a = ambiente("esquema-invalido");
    let id = a.id();
    erro_com(a.servico.esquema(&id, &OperacaoEsquema::ExcluirTabela { tabela: "fantasma".into() }, true), 404);
    let em_visao = OperacaoEsquema::AdicionarColuna { tabela: "v_pedidos_por_cliente".into(), coluna: col("x", "") };
    erro_com(a.servico.esquema(&id, &em_visao, true), 400);
}

#[test]
fn esquema_somente_leitura() {
    let a = ambiente_leitura("esquema-ro");
    let e =
        erro_com(a.servico.esquema(&a.id(), &OperacaoEsquema::ExcluirTabela { tabela: "eventos".into() }, true), 403);
    assert!(e.mensagem.contains("leitura"));
}

// ---- consulta livre ----------------------------------------------------------------------------------

#[test]
fn sql_com_historico() {
    let a = ambiente("sql");
    let id = a.id();
    let r = a.servico.sql(&id, "SELECT count(*) AS n FROM clientes", 1000).unwrap();
    assert_eq!(r.resultados[0].tipo(), "linhas");
    assert_eq!(linhas_de(&r.resultados[0]), &vec![vec![Celula::Inteiro(amostras::N_CLIENTES)]]);
    assert!(!r.alterou_dados);
    assert!(!r.transacao_aberta);

    let r = a.servico.sql(&id, "UPDATE eventos SET tipo = 'login' WHERE rowid = 1; SELECT 1", 1000).unwrap();
    assert_eq!(r.resultados.iter().map(|x| x.tipo()).collect::<Vec<_>>(), ["alteracao", "linhas"]);
    assert!(matches!(r.resultados[0], ResultadoComando::Alteracao { linhas_afetadas: Some(1), .. }));
    assert!(r.alterou_dados);

    let itens = a.servico.historico(&id).unwrap();
    let sqls: Vec<&str> = itens.iter().map(|i| i.sql.as_str()).collect();
    assert_eq!(
        sqls,
        ["UPDATE eventos SET tipo = 'login' WHERE rowid = 1; SELECT 1", "SELECT count(*) AS n FROM clientes"]
    );
    assert!(itens.iter().all(|i| i.quando.is_some()));
}

#[test]
fn sql_com_erro_nao_entra_no_historico() {
    let a = ambiente("sql-erro");
    let id = a.id();
    let r = a.servico.sql(&id, "SELECT * FROM fantasma", 1000).unwrap();
    match &r.resultados[0] {
        ResultadoComando::Erro { mensagem, .. } => assert!(!mensagem.is_empty()),
        outro => panic!("{outro:?}"),
    }
    let _ = a.servico.sql(&id, "   ", 1000);
    assert!(a.servico.historico(&id).unwrap().is_empty());
}

#[test]
fn sql_limite_e_esquema() {
    let a = ambiente("sql-limite");
    let r = a.servico.sql(&a.id(), "SELECT * FROM eventos", 10).unwrap();
    assert_eq!(linhas_de(&r.resultados[0]).len(), 10);
    assert!(matches!(r.resultados[0], ResultadoComando::Linhas { truncado: true, .. }));
    assert!(a.servico.sql(&a.id(), "CREATE TABLE temporaria (a)", 1000).unwrap().alterou_esquema);
}

#[test]
fn sql_transacao_aberta_trava_reconstrucao() {
    let a = ambiente("sql-transacao");
    let id = a.id();
    assert!(a.servico.sql(&id, "BEGIN", 1000).unwrap().transacao_aberta);
    let mut nova = col("observacao", "TEXT");
    nova.padrao = Some("''".into());
    let alterar = OperacaoEsquema::AlterarColuna { tabela: "pedidos".into(), coluna: "observacao".into(), nova };
    erro_com(a.servico.esquema(&id, &alterar, false), 409);
    assert!(!a.servico.sql(&id, "ROLLBACK", 1000).unwrap().transacao_aberta);
}

// ---- exportação --------------------------------------------------------------------------------------

#[test]
fn exportar_tabela() {
    let a = ambiente("exportar-tabela");
    for formato in Formato::TODOS {
        let destino = a.join(&format!("clientes.{}", extensao(formato)));
        let opcoes = Opcoes { csv_separador: ';', ..Opcoes::default() };
        let r = a.exportar_com(tabela("clientes"), formato, &destino, opcoes, false, None).unwrap();
        assert!(r.avisos.is_empty());
        assert_eq!(r.caminho, destino);
        assert_eq!(r.nome, format!("clientes.{}", extensao(formato)));
        assert_eq!(r.formato, formato);
        assert_eq!(r.linhas, N_CLIENTES);
        assert_eq!(r.tabelas, 1);
        assert_eq!(r.bytes, fs::metadata(&destino).unwrap().len());
        assert_eq!(ler_exportado(&destino, formato).values().sum::<usize>() as u64, N_CLIENTES, "{formato}");
    }
}

#[test]
fn exportar_consulta() {
    let a = ambiente("exportar-consulta");
    for formato in Formato::TODOS {
        let destino = a.join(&format!("consulta.{}", extensao(formato)));
        let r = a.exportar(consulta("SELECT codigo, descricao FROM produtos"), formato, &destino, None).unwrap();
        assert_eq!(r.linhas, N_PRODUTOS);
        assert_eq!(ler_exportado(&destino, formato).values().sum::<usize>() as u64, N_PRODUTOS, "{formato}");
    }
}

#[test]
fn exportar_banco() {
    let a = ambiente("exportar-banco");
    for formato in Formato::TODOS {
        let destino = a.join(&format!("banco.{}", extensao(formato)));
        let r = a.exportar(banco_com(&["clientes", "produtos"]), formato, &destino, None).unwrap();
        assert_eq!(r.tabelas, 2);
        assert_eq!(r.linhas, N_CLIENTES + N_PRODUTOS);
        if formato == Formato::Csv {
            assert_eq!(r.caminho.extension().unwrap(), "zip");
            assert_eq!(r.nome, "banco.zip");
        }
        assert!(r.caminho.exists());
        let conteudo = ler_exportado(&r.caminho, formato);
        let esperado = BTreeMap::from([
            ("clientes".to_string(), N_CLIENTES as usize),
            ("produtos".to_string(), N_PRODUTOS as usize),
        ]);
        assert_eq!(conteudo, esperado, "{formato}");
        if formato == Formato::Json {
            assert_eq!(ler_json(&r.caminho).campo("banco").texto(), "exemplo.db");
        }
    }
}

#[test]
fn exportar_banco_inteiro() {
    let a = ambiente("exportar-tudo");
    let destino = a.join("tudo.json");
    let opcoes = Opcoes { json_indentado: false, ..Opcoes::default() };
    let r = a.exportar_com(OrigemExportacao::Banco { tabelas: None }, Formato::Json, &destino, opcoes, false, None);
    assert_eq!(r.unwrap().tabelas, TABELAS_EXEMPLO.len());
    let dados = ler_json(&destino);
    let tabelas = dados.campo("tabelas");
    assert_eq!(
        tabelas.chaves().into_iter().collect::<HashSet<_>>(),
        TABELAS_EXEMPLO.iter().map(|t| t.to_string()).collect()
    );
    assert_eq!(tabelas.campo("eventos").lista().len() as u64, N_EVENTOS);
}

#[test]
fn exportar_tabela_filtrada_ordenada_e_marcada() {
    let a = ambiente("exportar-filtrada");
    let destino = a.join("pedidos.json");
    let filtro = FiltroExportacao {
        filtros: vec![banco::Filtro::texto("status", Operador::Igual, "pago")],
        ordem: Some("id".into()),
        direcao: Direcao::Desc,
        ..FiltroExportacao::default()
    };
    let r = a.exportar(OrigemExportacao::Tabela { nome: "pedidos".into(), filtro }, Formato::Json, &destino, None);
    let r = r.unwrap();
    let json = ler_json(&destino);
    let linhas = json.lista();
    assert!(!linhas.is_empty());
    assert_eq!(r.linhas, linhas.len() as u64);
    assert!(linhas.iter().all(|l| l.campo("status").texto() == "pago"));
    let ids: Vec<i64> = linhas.iter().map(|l| l.campo("id").inteiro()).collect();
    let mut decrescentes = ids.clone();
    decrescentes.sort_by(|x, y| y.cmp(x));
    assert_eq!(ids, decrescentes);

    let filtro = FiltroExportacao {
        chaves: Some(vec![Chave::Rowid(3), Chave::Rowid(7)]),
        busca: Some("example".into()),
        ..FiltroExportacao::default()
    };
    // o mesmo destino de novo: substituir precisa vir confirmado (pela pessoa ou pelo diálogo de salvar)
    let origem = OrigemExportacao::Tabela { nome: "clientes".into(), filtro };
    let r = a.exportar_com(origem, Formato::Json, &destino, Opcoes::default(), true, None).unwrap();
    assert_eq!(r.linhas, 2);
    let ids: Vec<i64> = ler_json(&destino).lista().iter().map(|l| l.campo("id").inteiro()).collect();
    assert_eq!(ids, [3, 7]);
}

#[test]
fn exportar_opcoes_viram_padrao() {
    let a = ambiente("exportar-opcoes");
    let destino = a.join("eventos.csv");
    let opcoes = Opcoes {
        cabecalho: false,
        csv_separador: '\t',
        csv_decimal: '.',
        json_indentado: false,
        csv_proteger_formulas: true,
    };
    a.exportar_com(tabela("eventos"), Formato::Csv, &destino, opcoes, false, None).unwrap();
    let texto = sem_bom(&fs::read(&destino).unwrap());
    let primeira = texto.lines().next().unwrap();
    assert!(primeira.contains('\t') && !primeira.starts_with("momento"));
    // as cinco opções do pedido viram o padrão (a janela Exportar sempre manda as cinco)
    assert_eq!(
        a.servico.prefs().exportacao(),
        OpcoesExportacao {
            csv_separador: '\t',
            csv_decimal: '.',
            json_indentado: false,
            cabecalho: Some(false),
            csv_proteger_formulas: Some(true),
        }
    );
    assert_eq!(a.servico.estado().unwrap().prefs.exportacao.csv_decimal, '.');
}

#[test]
fn exportar_erros() {
    let a = ambiente("exportar-erros");
    let destino = a.join("a.xlsx");
    erro_com(a.exportar(tabela("clientes"), Formato::Xlsx, &a.join("nao").join("a.xlsx"), None), 404);
    erro_com(a.exportar(tabela("clientes"), Formato::Xlsx, &a.pasta.caminho, None), 400);
    erro_com(a.exportar(tabela("fantasma"), Formato::Xlsx, &destino, None), 404);
    erro_com(a.exportar(OrigemExportacao::Banco { tabelas: Some(vec![]) }, Formato::Xlsx, &destino, None), 400);
    erro_com(a.exportar(consulta("DELETE FROM clientes"), Formato::Xlsx, &destino, None), 400);
    let barra = Opcoes { csv_separador: '|', ..Opcoes::default() };
    erro_com(a.exportar_com(tabela("clientes"), Formato::Xlsx, &destino, barra, false, None), 400);
    erro_com(a.exportar(tabela("clientes"), Formato::Xlsx, Path::new("  "), None), 400);
    assert!(!destino.exists());
    assert_eq!(a.servico.contagem(&a.id(), "clientes", None, &[], None).unwrap().linhas, N_CLIENTES);
}

#[test]
fn exportar_libera_o_banco() {
    let a = ambiente("exportar-libera");
    a.exportar(OrigemExportacao::Banco { tabelas: None }, Formato::Xlsx, &a.join("tudo.xlsx"), None).unwrap();
    // nenhuma conexão de leitura ficou aberta segurando o arquivo: a gravação passa na hora
    a.servico.inserir(&a.id(), "eventos", &vals(&[("tipo", tx("teste"))])).unwrap();
}

// ---- regressões dos achados da revisão ---------------------------------------------------------------

#[test]
fn transacao_aberta_marca_as_gravacoes_e_impede_fechar_sem_querer() {
    let a = ambiente("transacao");
    let id = a.id();
    assert!(a.servico.sql(&id, "BEGIN", 1000).unwrap().transacao_aberta);
    let r = a.servico.atualizar(&id, "clientes", &Chave::Rowid(1), &vals(&[("cidade", tx("Natal"))])).unwrap();
    assert!(r.transacao_aberta);
    assert!(a.servico.inserir(&id, "eventos", &vals(&[("tipo", tx("teste"))])).unwrap().transacao_aberta);
    let r = a.servico.excluir(&id, "eventos", &[Chave::Rowid(1)]).unwrap();
    assert_eq!((r.resultado, r.transacao_aberta), (1, true));
    let r = a.servico.esquema(&id, &adicionar_origem(None), false).unwrap();
    assert!(r.transacao_aberta);
    assert!(a.servico.estado().unwrap().banco.unwrap().transacao_aberta);

    let novo = a.join("novo.db");
    for r in [
        a.servico.fechar_banco(&id, false).map(|_| ()),
        a.servico.abrir_banco(&a.exemplo, false, false).map(|_| ()),
        a.servico.novo_banco(&novo, false).map(|_| ()),
    ] {
        let e = erro_com(r, 409);
        assert!(e.tem_codigo(Codigo::TransacaoAberta) && e.mensagem.contains("COMMIT"), "{e:?}");
    }
    assert!(!novo.exists());
    assert_eq!(a.servico.info_banco(&id).unwrap().nome, "exemplo.db");

    assert!(!a.servico.sql(&id, "COMMIT", 1000).unwrap().transacao_aberta);
    let r = a.servico.atualizar(&id, "clientes", &Chave::Rowid(2), &vals(&[("cidade", tx("X"))])).unwrap();
    assert!(!r.transacao_aberta);
    a.servico.fechar_banco(&id, false).unwrap();
    assert_eq!(cru(&a.exemplo, "SELECT cidade FROM clientes WHERE id = 1"), [[Valor::Texto("Natal".into())]]);
}

#[test]
fn fechar_descartando_a_transacao_por_escolha() {
    let a = ambiente("descartar");
    a.servico.sql(&a.id(), "BEGIN; UPDATE clientes SET cidade = 'Natal' WHERE id = 1;", 1000).unwrap();
    a.servico.fechar_banco(&a.id(), true).unwrap();
    assert_ne!(cru(&a.exemplo, "SELECT cidade FROM clientes WHERE id = 1"), [[Valor::Texto("Natal".into())]]);
}

#[test]
fn pedido_de_outra_janela_para_outro_banco_e_recusado() {
    // a tela que ainda mostra o banco A não pode gravar no B
    let a = ambiente("outra-janela");
    let id_a = a.id();
    let outro = a.join("outro.db");
    rusqlite::Connection::open(&outro)
        .unwrap()
        .execute_batch(
            "CREATE TABLE clientes (id INTEGER PRIMARY KEY, nome TEXT); INSERT INTO clientes VALUES (1, 'B');",
        )
        .unwrap();
    let id_b = a.servico.abrir_banco(&outro, false, false).unwrap().id;
    assert_ne!(id_b, id_a);
    let edicao = vals(&[("nome", tx("Editado na janela A"))]);
    let e = erro_com(a.servico.atualizar(&id_a, "clientes", &Chave::Rowid(1), &edicao), 409);
    assert!(e.tem_codigo(Codigo::BancoTrocado));
    assert_eq!(e.mensagem, crate::MSG_BANCO_TROCADO);
    let e = erro_com(a.servico.linhas(&id_a, "clientes", &PedidoPagina::default()), 409);
    assert!(e.tem_codigo(Codigo::BancoTrocado));
    for e in [
        erro(a.servico.info_banco(&id_a)),
        erro(a.servico.sql(&id_a, "DELETE FROM clientes", 1000)),
        erro(a.servico.exportar(
            &id_a,
            &crate::PedidoExportacao {
                origem: tabela("clientes"),
                formato: Formato::Csv,
                caminho: a.join("x.csv"),
                opcoes: Opcoes::default(),
                substituir: false,
            },
            None,
        )),
    ] {
        assert!(e.tem_codigo(Codigo::BancoTrocado), "{e:?}");
    }
    assert!(!a.join("x.csv").exists());
    // "Fechar banco" da tela que ainda mostra o A não fecha o B, nem desfaz a transação aberta nele
    a.servico.sql(&id_b, "BEGIN; UPDATE clientes SET nome = 'na transação do B' WHERE id = 1;", 1000).unwrap();
    for descartar in [false, true] {
        let e = erro_com(a.servico.fechar_banco(&id_a, descartar), 409);
        assert!(e.tem_codigo(Codigo::BancoTrocado), "{e:?}");
        let info = a.servico.info_banco(&id_b).unwrap();
        assert!(info.nome == "outro.db" && info.transacao_aberta, "{info:?}");
    }
    a.servico.sql(&id_b, "ROLLBACK", 1000).unwrap();
    assert_eq!(cru(&outro, "SELECT nome FROM clientes"), [[Valor::Texto("B".into())]]);
    // com a identidade certa, grava
    a.servico.atualizar(&id_b, "clientes", &Chave::Rowid(1), &edicao).unwrap();
    // fechado o banco, quem ainda mostra o B também é avisado (e não "Nenhum banco aberto.")
    a.servico.fechar_banco(&id_b, false).unwrap();
    assert!(erro_com(a.servico.info_banco(&id_b), 409).tem_codigo(Codigo::BancoTrocado));
    assert!(a.servico.estado().is_ok()); // estado sempre responde
}

#[test]
fn destinos_que_estragariam_o_banco_ou_o_app_sao_recusados() {
    // exportar/BLOB/cópia no "<banco>-journal": o SQLite apagava o arquivo e, só leitura, o banco parava de abrir
    let a = ambiente("destinos");
    let id = a.id();
    a.servico.gravar_prefs(Some(Tema::Escuro), None).unwrap(); // o arquivo de preferências existe
    let exemplo = a.exemplo.display().to_string();
    let destinos = [
        a.exemplo.clone(),
        PathBuf::from(format!("{exemplo}-journal")),
        PathBuf::from(format!("{exemplo}-wal")),
        PathBuf::from(format!("{}-SHM", exemplo.to_uppercase())),
        a.servico.prefs().arquivo().to_path_buf(),
        a.servico.prefs().arquivo_trava(),
    ];
    for destino in &destinos {
        let r = [
            erro(a.exportar_com(tabela("clientes"), Formato::Csv, destino, Opcoes::default(), true, None)),
            erro(a.servico.salvar_blob(&id, "produtos", &pk("P003"), "foto", destino, true)),
            erro(a.servico.copia(&id, destino, true)),
        ];
        for e in r {
            assert_eq!(e.status, 400, "{destino:?}: {e:?}");
            assert!(e.mensagem.contains("escolha outro nome") || e.mensagem.contains("outro arquivo"), "{e:?}");
        }
        assert!(!PathBuf::from(format!("{exemplo}-journal")).exists());
    }
    assert!(a.servico.linhas(&id, "clientes", &PedidoPagina::default()).is_ok());
    assert_eq!(a.servico.prefs().tema(), Tema::Escuro);
}

#[test]
fn registro_do_aplicativo_tambem_e_recusado() {
    // o editor.log e as cópias da rotação
    let a = ambiente("registro");
    let log = a.join("editor.log");
    let servico = Servico::novo(prefs_em(&a.pasta)).com_arquivo_log(log.clone());
    let info = servico.abrir_banco(&a.exemplo, false, false).unwrap();
    for destino in [log.clone(), a.join("editor.log.3")] {
        let pedido = crate::PedidoExportacao {
            origem: tabela("clientes"),
            formato: Formato::Csv,
            caminho: destino,
            opcoes: Opcoes::default(),
            substituir: true,
        };
        assert_eq!(erro_com(servico.exportar(&info.id, &pedido, None), 400).mensagem, crate::MSG_DESTINO_DO_APP);
    }
    drop(servico);
}

#[test]
fn arquivo_existente_so_e_substituido_com_confirmacao() {
    // sem a confirmação (da pessoa ou do diálogo de salvar), o caminho digitado trocava um arquivo existente
    let a = ambiente("existente");
    let id = a.id();
    let existente = a.join("relatorio.csv");
    let original = b"conteudo importante que a pessoa ja tinha\r\n";
    fs::write(&existente, original).unwrap();
    let e = erro_com(a.exportar(tabela("clientes"), Formato::Csv, &existente, None), 409);
    assert!(e.tem_codigo(Codigo::DestinoExiste) && e.mensagem.contains("relatorio.csv"), "{e:?}");
    let e = erro_com(a.servico.salvar_blob(&id, "produtos", &pk("P003"), "foto", &existente, false), 409);
    assert!(e.tem_codigo(Codigo::DestinoExiste));
    assert!(erro_com(a.servico.copia(&id, &existente, false), 409).tem_codigo(Codigo::DestinoExiste));
    assert_eq!(fs::read(&existente).unwrap(), original);

    a.exportar_com(tabela("clientes"), Formato::Csv, &existente, Opcoes::default(), true, None).unwrap();
    assert!(fs::read(&existente).unwrap().starts_with(b"\xef\xbb\xbfid;nome"));
    // de novo sem confirmar: recusado outra vez
    erro_com(a.exportar(tabela("clientes"), Formato::Csv, &existente, None), 409);
}

#[test]
fn exportar_banco_inteiro_em_csv_nao_substitui_zip_sem_perguntar() {
    let a = ambiente("zip-existente");
    let existente = a.join("relatorio.zip");
    fs::write(&existente, b"um .zip importante").unwrap();
    let pedido = a.join("relatorio.csv");
    let todas = || OrigemExportacao::Banco { tabelas: None };
    assert!(erro_com(a.exportar(todas(), Formato::Csv, &pedido, None), 409).tem_codigo(Codigo::DestinoExiste));
    let e = erro_com(a.exportar_com(todas(), Formato::Csv, &pedido, Opcoes::default(), true, None), 409);
    assert!(e.tem_codigo(Codigo::DestinoExiste)); // confirmar o .csv não confirma o .zip
    assert_eq!(fs::read(&existente).unwrap(), b"um .zip importante");
}

#[test]
fn copia_com_transacao_aberta_responde_na_hora() {
    // a API de backup repetia para sempre segurando a trava: o aplicativo inteiro congelava
    let a = ambiente("copia-transacao");
    let id = a.id();
    a.servico.sql(&id, "BEGIN; UPDATE clientes SET cidade = 'Natal' WHERE id = 1;", 1000).unwrap();
    let inicio = Instant::now();
    let e = erro_com(a.servico.copia(&id, &a.join("copia.db"), false), 409);
    assert!(e.tem_codigo(Codigo::TransacaoAberta) && inicio.elapsed().as_secs_f64() < 2.0);
    assert!(a.servico.estado().is_ok());
    a.servico.sql(&id, "ROLLBACK", 1000).unwrap();
}

#[test]
fn exportar_com_transacao_aberta_sai_igual_a_grade() {
    let a = ambiente("exportar-transacao");
    let id = a.id();
    a.servico
        .sql(
            &id,
            "BEGIN; UPDATE clientes SET cidade = 'Natal' WHERE id = 1;\
             INSERT INTO clientes (id, nome, email) VALUES (999, 'Nova', 'n@example.test');",
            1000,
        )
        .unwrap();
    let destino = a.join("clientes.json");
    a.exportar(tabela("clientes"), Formato::Json, &destino, None).unwrap();
    let linhas = ler_json(&destino);
    assert_eq!(linhas.lista().len() as u64, N_CLIENTES + 1);
    assert_eq!(linhas.lista()[0].campo("cidade").texto(), "Natal");
    let destino = a.join("consulta.json");
    a.exportar(consulta("SELECT id, cidade FROM clientes WHERE id IN (1, 999)"), Formato::Json, &destino, None)
        .unwrap();
    let pares: Vec<(i64, String)> = ler_json(&destino)
        .lista()
        .iter()
        .map(|l| (l.campo("id").inteiro(), l.campo("cidade").texto().into()))
        .collect();
    // São Paulo: o DEFAULT da coluna
    assert_eq!(pares, [(1, "Natal".to_string()), (999, "São Paulo".to_string())]);
    a.servico.sql(&id, "ROLLBACK", 1000).unwrap();
}

#[test]
fn exportar_consulta_de_tabela_temporaria() {
    let a = ambiente("temporaria");
    let id = a.id();
    let roteiro = "CREATE TEMP TABLE resumo AS SELECT cidade, count(*) AS n FROM clientes GROUP BY cidade; \
                   SELECT * FROM resumo";
    let r = a.servico.sql(&id, roteiro, 1000).unwrap();
    let n = linhas_de(r.resultados.last().unwrap()).len() as u64;
    let r = a.exportar(consulta("SELECT * FROM resumo"), Formato::Json, &a.join("resumo.json"), None).unwrap();
    assert_eq!(r.linhas, n);
}

#[test]
fn exportar_banco_sem_tabelas_explica() {
    let pasta = Pasta::nova("sem-tabelas");
    let vazio = amostras::criar_banco_vazio(pasta.join("vazio.db")).unwrap();
    let a = ambiente_de(pasta, &vazio);
    let e = erro_com(
        a.exportar(OrigemExportacao::Banco { tabelas: None }, Formato::Xlsx, &a.join("vazio.xlsx"), None),
        400,
    );
    assert!(e.mensagem.contains("não tem tabelas"), "{e:?}");
}

#[test]
fn exportar_excel_devolve_avisos() {
    let a = ambiente("avisos");
    a.servico
        .sql(
            &a.id(),
            "CREATE TABLE anexos (id INTEGER PRIMARY KEY, dados BLOB); INSERT INTO anexos VALUES (1, zeroblob(60000));",
            1000,
        )
        .unwrap();
    let r = a.exportar(tabela("anexos"), Formato::Xlsx, &a.join("anexos.xlsx"), None).unwrap();
    assert_eq!(r.avisos.len(), 1);
    assert!(r.avisos[0].contains("BLOB"));
}

#[test]
fn filtros_invalidos_na_query_sao_400() {
    // a parte dos filtros em JSON não existe aqui (são tipados); a página enorme continua respondendo vazio
    let a = ambiente("pagina-enorme");
    let pedido = PedidoPagina { pagina: 10u64.pow(19), ..PedidoPagina::default() };
    assert!(a.servico.linhas(&a.id(), "clientes", &pedido).unwrap().linhas.is_empty());
}
