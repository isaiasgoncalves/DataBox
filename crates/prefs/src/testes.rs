//! Testes portados de `tests/test_prefs.py` (mesmos nomes, sem o prefixo `test_`) e alguns do porte.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{NaiveDateTime, TimeZone};

use super::*;

// ---- apoio ------------------------------------------------------------------------------------------

/// Pasta temporária própria do teste, apagada no fim.
pub(crate) struct PastaTemp(PathBuf);

impl PastaTemp {
    pub(crate) fn nova(nome: &str) -> PastaTemp {
        static N: AtomicU64 = AtomicU64::new(0);
        let caminho = std::env::temp_dir().join(format!(
            "editor-prefs-{nome}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&caminho);
        fs::create_dir_all(&caminho).unwrap();
        PastaTemp(caminho)
    }

    pub(crate) fn caminho(&self) -> &Path {
        &self.0
    }

    /// `<tmp>/config/preferencias.json`.
    pub(crate) fn arquivo(&self) -> PathBuf {
        self.0.join("config").join("preferencias.json")
    }
}

impl Drop for PastaTemp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn abrir(arquivo: &Path) -> Preferencias {
    Preferencias::abrir(Some(arquivo.to_path_buf()))
}

fn banco(pasta: &Path, nome: &str, conteudo: &[u8]) -> PathBuf {
    let caminho = pasta.join(nome);
    fs::create_dir_all(pasta).unwrap();
    fs::write(&caminho, conteudo).unwrap();
    caminho
}

fn iso(texto: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(texto, "%Y-%m-%dT%H:%M:%S").unwrap()
}

fn segundos_de_agora(texto: &str) -> i64 {
    (iso(texto) - chrono::Local::now().naive_local()).num_seconds().abs()
}

fn texto(caminho: &Path) -> String {
    caminho.to_string_lossy().into_owned()
}

fn json_do_arquivo(arquivo: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(arquivo).unwrap()).unwrap()
}

/// O arquivo pelo leitor do crate, que guarda a ordem das chaves e aceita `NaN` (o `serde_json` não).
fn json_lido(arquivo: &Path) -> Json {
    json::ler(&fs::read(arquivo).unwrap()).unwrap()
}

/// As chaves de um objeto, na ordem.
fn chaves(objeto: &Json) -> Vec<&str> {
    match objeto {
        Json::Objeto(pares) => pares.iter().map(|(k, _)| k.as_str()).collect(),
        outro => panic!("não é objeto: {outro:?}"),
    }
}

/// Os itens de uma lista.
fn itens(lista: Option<&Json>) -> &[Json] {
    match lista {
        Some(Json::Lista(itens)) => itens,
        outro => panic!("não é lista: {outro:?}"),
    }
}

fn texto_json(t: &str) -> Json {
    Json::Texto(t.to_string())
}

/// Grava `conteudo` em `arquivo`, criando a pasta.
fn gravar_texto(arquivo: &Path, conteudo: &str) {
    fs::create_dir_all(arquivo.parent().unwrap()).unwrap();
    fs::write(arquivo, conteudo).unwrap();
}

/// Os dados sem as chaves que esta versão não conhece e sem a aparência e os painéis: o arquivo de quem nunca
/// escolheu nada disso.
pub(crate) fn sem_extras(dados: &Dados) -> Dados {
    let mut limpos = dados.clone();
    limpos.extras.clear();
    limpos.aparencia = None;
    limpos.paineis = None;
    limpos.pasta_aberta = None; // também só do DataBox
    for recente in &mut limpos.recentes {
        recente.extras.clear();
    }
    limpos
}

fn nomes(itens: &[Recente]) -> Vec<&str> {
    itens.iter().map(|i| i.nome.as_str()).collect()
}

fn sqls(itens: &[ComandoSql]) -> Vec<&str> {
    itens.iter().map(|i| i.sql.as_str()).collect()
}

// ---- padrão e local do arquivo ----------------------------------------------------------------------

#[test]
fn sem_arquivo_vale_o_padrao_e_nada_e_gravado() {
    let tmp = PastaTemp::nova("padrao");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    assert_eq!(p.tema(), Tema::Sistema);
    assert_eq!(p.exportacao(), OpcoesExportacao::default());
    assert_eq!(p.ultimo_diretorio(), None);
    assert!(p.recentes().is_empty());
    assert_eq!(
        p.resumo(),
        PrefsInfo {
            tema: Tema::Sistema,
            recentes: vec![],
            exportacao: OpcoesExportacao::default(),
            aparencia: Aparencia::default()
        }
    );
    assert!(!arquivo.exists());
}

#[test]
fn arquivo_padrao_fica_em_appdata_databox() {
    let tmp = PastaTemp::nova("appdata");
    let roaming = tmp.caminho().join("Roaming");
    assert_eq!(
        arquivo_padrao_de(Some(roaming.clone().into_os_string()), || unreachable!()),
        roaming.join("DataBox").join("preferencias.json")
    );
    // o arquivo_padrao() de verdade segue a mesma regra com o APPDATA do processo (só calcula: não toca no disco)
    if cfg!(windows) {
        assert_eq!(arquivo_padrao(), arquivo_padrao_de(std::env::var_os("APPDATA"), comum::app::pasta_pessoal));
    }
    assert_eq!(Preferencias::abrir(None).arquivo(), arquivo_padrao());
}

#[test]
fn sem_appdata_usa_a_pasta_pessoal() {
    let tmp = PastaTemp::nova("casa");
    let casa = tmp.caminho().join("casa");
    let esperado = casa.join("DataBox").join("preferencias.json");
    assert_eq!(arquivo_padrao_de(None, || casa.clone()), esperado);
    assert_eq!(arquivo_padrao_de(Some("".into()), || casa.clone()), esperado);
}

#[test]
fn exportacao_devolvida_e_copia() {
    let tmp = PastaTemp::nova("copia");
    let p = abrir(&tmp.arquivo());
    let mut copia = p.exportacao();
    copia.csv_separador = ',';
    assert_eq!(copia.csv_separador, ',');
    assert_eq!(p.exportacao().csv_separador, ';');
}

// ---- recentes ---------------------------------------------------------------------------------------

#[test]
fn recentes_mais_novo_primeiro_com_estado_do_arquivo() {
    let tmp = PastaTemp::nova("recentes");
    let p = abrir(&tmp.arquivo());
    let vendas = banco(&tmp.caminho().join("dados"), "vendas.db", &[b'a'; 1234]);
    let estoque = banco(&tmp.caminho().join("dados"), "estoque.sqlite", &[b'b'; 10]);
    p.adicionar_recente(&vendas);
    p.adicionar_recente(&estoque);

    let itens = p.recentes();
    assert_eq!(nomes(&itens), ["estoque.sqlite", "vendas.db"]);
    let (primeiro, segundo) = (&itens[0], &itens[1]);
    assert_eq!(primeiro.caminho, caminho_exibido(&estoque));
    assert_eq!(primeiro.caminho, texto(&estoque));
    assert_eq!(primeiro.pasta, texto(&tmp.caminho().join("dados")));
    assert!(primeiro.existe);
    assert_eq!(segundo.tamanho, Some(1234));
    let mtime = fs::metadata(&vendas).unwrap().modified().unwrap();
    let esperado = chrono::DateTime::<chrono::Local>::from(mtime).naive_local();
    let esperado = esperado.and_utc().timestamp();
    assert_eq!(iso(segundo.modificado_em.as_deref().unwrap()).and_utc().timestamp(), esperado);
    assert!(segundos_de_agora(segundo.aberto_em.as_deref().unwrap()) < 60);
}

#[test]
fn recentes_calcula_existe_e_tamanho_na_hora() {
    let tmp = PastaTemp::nova("na-hora");
    let p = abrir(&tmp.arquivo());
    let b = banco(tmp.caminho(), "vendas.db", b"123");
    p.adicionar_recente(&b);
    assert_eq!(p.recentes()[0].tamanho, Some(3));
    fs::write(&b, b"123456").unwrap();
    assert_eq!(p.recentes()[0].tamanho, Some(6));
    fs::remove_file(&b).unwrap();
    let item = &p.recentes()[0];
    assert!(!item.existe && item.tamanho.is_none() && item.modificado_em.is_none());
}

#[test]
fn pasta_com_nome_de_banco_nao_conta_como_existente() {
    let tmp = PastaTemp::nova("falso");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    fs::create_dir(tmp.caminho().join("falso.db")).unwrap();
    // um "tipo": "banco" escrito é respeitado: uma pasta não é o arquivo comum que um banco precisa ser
    let caminho = serde_json::to_string(&texto(&tmp.caminho().join("falso.db"))).unwrap();
    gravar_texto(&arquivo, &format!(r#"{{"recentes": [{{"caminho": {caminho}, "tipo": "banco"}}]}}"#));
    let item = &p.recentes()[0];
    assert_eq!(item.tipo, TipoRecente::Banco);
    assert!(!item.existe && item.tamanho.is_none() && item.modificado_em.is_none());
}

/// Um recente que hoje é uma pasta, gravado sem `"tipo"` (como um banco).
#[test]
fn sem_tipo_e_pasta_no_disco_vale_como_pasta() {
    let tmp = PastaTemp::nova("sem-tipo-pasta");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    let pasta = tmp.caminho().join("Projetos.db"); // até o nome de banco não muda o que é no disco
    fs::create_dir(&pasta).unwrap();
    let vendas = banco(tmp.caminho(), "vendas.db", b"123");
    p.adicionar_recente(&vendas);
    p.adicionar_recente(&pasta); // sem "tipo" (banco)
    p.adicionar_recente(tmp.caminho().join("sumiu.db"));
    assert!(!fs::read_to_string(&arquivo).unwrap().contains("\"tipo\""));

    let itens = p.recentes();
    let vistos: Vec<(&str, TipoRecente, bool)> = itens.iter().map(|i| (i.nome.as_str(), i.tipo, i.existe)).collect();
    assert_eq!(
        vistos,
        [
            ("sumiu.db", TipoRecente::Banco, false), // não existe: continua banco
            ("Projetos.db", TipoRecente::Pasta, true),
            ("vendas.db", TipoRecente::Banco, true)
        ]
    );
    assert_eq!(itens[1].tamanho, None, "a pasta não tem tamanho");
    assert!(itens[1].modificado_em.is_some());
    // a consulta não escreve nada no arquivo: o tipo vem do disco, na hora
    assert!(!fs::read_to_string(&arquivo).unwrap().contains("\"tipo\""));

    // o que era pasta e virou arquivo (ou sumiu e voltou como arquivo) volta a ser banco
    fs::remove_dir(&pasta).unwrap();
    fs::write(&pasta, b"x").unwrap();
    let itens = p.recentes();
    assert_eq!((itens[1].tipo, itens[1].existe, itens[1].tamanho), (TipoRecente::Banco, true, Some(1)));

    // e um "tipo": "arquivo" num caminho que é uma pasta no disco é respeitado (e não existe: é pasta, não arquivo)
    let outra = tmp.caminho().join("outra");
    fs::create_dir(&outra).unwrap();
    p.adicionar_recente_de(&outra, TipoRecente::Arquivo);
    let item = &p.recentes()[0];
    assert_eq!((item.tipo, item.existe, item.tamanho), (TipoRecente::Arquivo, false, None));
}

#[test]
fn existe_da_pasta_e_ser_pasta() {
    let tmp = PastaTemp::nova("existe-pasta");
    let p = abrir(&tmp.arquivo());
    let pasta = tmp.caminho().join("Projetos");
    fs::create_dir(&pasta).unwrap();
    let dados = banco(tmp.caminho(), "dados.csv", b"a;b\n1;2\n");
    p.adicionar_recente_de(&pasta, TipoRecente::Pasta);
    p.adicionar_recente_de(&dados, TipoRecente::Arquivo);
    let itens = p.recentes();
    assert_eq!(nomes(&itens), ["dados.csv", "Projetos"]);
    assert_eq!((itens[0].tipo, itens[0].existe, itens[0].tamanho), (TipoRecente::Arquivo, true, Some(8)));
    assert_eq!((itens[1].tipo, itens[1].existe, itens[1].tamanho), (TipoRecente::Pasta, true, None));
    assert!(itens[0].modificado_em.is_some() && itens[1].modificado_em.is_some());
    assert_eq!(itens[1].pasta, texto(tmp.caminho()));

    // a pasta declarada que virou arquivo comum não "existe"; o arquivo declarado que virou pasta também não
    fs::remove_dir(&pasta).unwrap();
    fs::write(&pasta, b"x").unwrap();
    fs::remove_file(&dados).unwrap();
    fs::create_dir(&dados).unwrap();
    for item in p.recentes() {
        assert!(!item.existe && item.tamanho.is_none() && item.modificado_em.is_none(), "{}", item.nome);
    }
    // e a que não existe mais, também não
    fs::remove_file(&pasta).unwrap();
    assert!(!p.recentes()[1].existe);
}

// Regra do Windows (caminhos `C:\…`, a mesma chave com outra caixa ou barra, o JSON gravado com `\\`):
// fora dele, os caminhos seguem o posixpath (`caminhos::testes::fora_do_windows_e_o_posixpath`).
#[test]
#[cfg(windows)]
fn recente_repetido_sobe_sem_duplicar_mesmo_com_caixa_e_barras_diferentes() {
    let tmp = PastaTemp::nova("repetido");
    let p = abrir(&tmp.arquivo());
    let a = banco(&tmp.caminho().join("Dados"), "Vendas.db", b"x");
    let b = banco(&tmp.caminho().join("Dados"), "outro.db", b"x");
    p.adicionar_recente(&a);
    p.adicionar_recente(&b);
    let outra_grafia = texto(&a).to_uppercase().replace('\\', "/");
    p.adicionar_recente(&outra_grafia);

    let itens = p.recentes();
    assert_eq!(itens.len(), 2);
    assert_eq!(caminhos::normcase(&itens[0].caminho), caminhos::normcase(&texto(&a)));
    // guarda como a pessoa escreveu da última vez (abspath, sem normcase: a caixa fica como veio)
    assert_eq!(itens[0].caminho, caminho_exibido(&outra_grafia));
    assert_eq!(itens[1].nome, "outro.db");
}

#[test]
fn caminho_relativo_vira_absoluto() {
    // sem trocar a pasta atual (os testes rodam em paralelo no mesmo processo)
    let tmp = PastaTemp::nova("relativo");
    let p = abrir(&tmp.arquivo());
    let nome = format!("rel_{}.db", tmp.caminho().file_name().unwrap().to_string_lossy());
    p.adicionar_recente(&nome);
    assert_eq!(p.recentes()[0].caminho, texto(&std::env::current_dir().unwrap().join(&nome)));
}

#[test]
fn no_maximo_15_recentes() {
    let tmp = PastaTemp::nova("quinze");
    let p = abrir(&tmp.arquivo());
    for i in 0..MAX_RECENTES + 5 {
        p.adicionar_recente(tmp.caminho().join(format!("banco{i:02}.db")));
    }
    let itens = p.recentes();
    assert_eq!(itens.len(), 15);
    assert_eq!(MAX_RECENTES, 15);
    assert_eq!(itens[0].nome, "banco19.db");
    assert_eq!(itens[14].nome, "banco05.db");
}

// Regra do Windows (caminhos `C:\…`, a mesma chave com outra caixa ou barra, o JSON gravado com `\\`):
// fora dele, os caminhos seguem o posixpath (`caminhos::testes::fora_do_windows_e_o_posixpath`).
#[test]
#[cfg(windows)]
fn remover_recente_normalizado() {
    let tmp = PastaTemp::nova("remover");
    let p = abrir(&tmp.arquivo());
    let (a, b) = (tmp.caminho().join("a.db"), tmp.caminho().join("b.db"));
    p.adicionar_recente(&a);
    p.adicionar_recente(&b);
    p.remover_recente(texto(&a).to_uppercase());
    assert_eq!(nomes(&p.recentes()), ["b.db"]);
    p.remover_recente(tmp.caminho().join("nunca_esteve.db")); // não é erro
    assert_eq!(p.recentes().len(), 1);
}

#[test]
fn recentes_nao_esperam_arquivo_lento() {
    let tmp = PastaTemp::nova("lento");
    let mut p = abrir(&tmp.arquivo());
    let rapido = banco(tmp.caminho(), "rapido.db", b"x");
    let demorado = tmp.caminho().join("rede").join("zz_demorado.db");
    p.adicionar_recente(&demorado);
    p.adicionar_recente(&rapido);
    let liberar = Arc::new(AtomicBool::new(false));
    let sinal = Arc::clone(&liberar);
    p.consulta = Arc::new(move |caminho: &str| {
        if caminho.ends_with("zz_demorado.db") {
            // uma unidade de rede desconectada demorando para responder
            let fim = Instant::now() + Duration::from_secs(5);
            while !sinal.load(Ordering::SeqCst) && Instant::now() < fim {
                thread::sleep(Duration::from_millis(10));
            }
        }
        estado_do_arquivo(caminho)
    });
    p.tempo_maximo_consulta = Duration::from_millis(300);
    let inicio = Instant::now();
    let itens = p.recentes();
    liberar.store(true, Ordering::SeqCst);
    assert!(inicio.elapsed() < Duration::from_secs(2));
    let pares: Vec<(&str, bool)> = itens.iter().map(|i| (i.nome.as_str(), i.existe)).collect();
    assert_eq!(pares, [("rapido.db", true), ("zz_demorado.db", false)]);
}

// ---- persistência e várias instâncias ---------------------------------------------------------------

// Regra do Windows (caminhos `C:\…`, a mesma chave com outra caixa ou barra, o JSON gravado com `\\`):
// fora dele, os caminhos seguem o posixpath (`caminhos::testes::fora_do_windows_e_o_posixpath`).
#[test]
#[cfg(windows)]
fn grava_e_outra_instancia_le() {
    let tmp = PastaTemp::nova("outra-le");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    p.atualizar(Mudanca {
        tema: Some(Tema::Escuro),
        exportacao: Some(MudancaExportacao { csv_separador: Some('\t'), ..Default::default() }),
        ..Default::default()
    })
    .unwrap();
    p.adicionar_recente(tmp.caminho().join("vendas.db"));
    p.registrar_sql(tmp.caminho().join("vendas.db"), "SELECT 1", None, None);

    assert_eq!(json_do_arquivo(&arquivo)["tema"], "dark");

    let outra = abrir(&arquivo);
    assert_eq!(outra.tema(), Tema::Escuro);
    assert_eq!(outra.exportacao(), OpcoesExportacao { csv_separador: '\t', ..Default::default() });
    assert_eq!(nomes(&outra.recentes()), ["vendas.db"]);
    assert_eq!(sqls(&outra.historico_sql(tmp.caminho().join("VENDAS.DB"))), ["SELECT 1"]);
}

#[test]
fn duas_janelas_abertas_nao_apagam_os_recentes_uma_da_outra() {
    let tmp = PastaTemp::nova("duas-janelas");
    let arquivo = tmp.arquivo();
    let (a, b) = (abrir(&arquivo), abrir(&arquivo));
    assert!(a.recentes().is_empty() && b.recentes().is_empty()); // as duas já leram o arquivo (vazio)
    a.adicionar_recente(tmp.caminho().join("um.db"));
    b.adicionar_recente(tmp.caminho().join("dois.db"));
    assert_eq!(nomes(&a.recentes()), ["dois.db", "um.db"]);
    assert_eq!(nomes(&b.recentes()), ["dois.db", "um.db"]);
}

#[test]
fn varias_threads_ao_mesmo_tempo() {
    let tmp = PastaTemp::nova("threads");
    let arquivo = tmp.arquivo();
    let p = Arc::new(abrir(&arquivo));
    let pasta = tmp.caminho().to_path_buf();
    let threads: Vec<_> = (0..6)
        .map(|n| {
            let p = Arc::clone(&p);
            let pasta = pasta.clone();
            thread::spawn(move || {
                for i in 0..10 {
                    p.adicionar_recente(pasta.join(format!("t{n}_{i}.db")));
                    p.registrar_sql(pasta.join("comum.db"), &format!("SELECT {}", n * 100 + i), None, None);
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let dados = json_do_arquivo(&arquivo);
    assert_eq!(dados["recentes"].as_array().unwrap().len(), MAX_RECENTES);
    assert_eq!(p.historico_sql(pasta.join("comum.db")).len(), MAX_COMANDOS);
}

// ---- histórico da Consulta SQL ----------------------------------------------------------------------

#[test]
fn historico_mais_recente_primeiro_sem_repetir() {
    let tmp = PastaTemp::nova("historico");
    let p = abrir(&tmp.arquivo());
    let b = tmp.caminho().join("vendas.db");
    p.registrar_sql(&b, "SELECT 1", None, None);
    p.registrar_sql(&b, "SELECT 2", None, None);
    p.registrar_sql(&b, "  SELECT 1  \n", None, None); // repetido (com espaços em volta) sobe para o topo
    let itens = p.historico_sql(&b);
    assert_eq!(sqls(&itens), ["SELECT 1", "SELECT 2"]);
    assert!(segundos_de_agora(itens[0].quando.as_deref().unwrap()) < 60);
}

#[test]
fn historico_no_maximo_30_por_banco() {
    let tmp = PastaTemp::nova("trinta");
    let p = abrir(&tmp.arquivo());
    let b = tmp.caminho().join("vendas.db");
    for i in 0..MAX_COMANDOS + 7 {
        p.registrar_sql(&b, &format!("SELECT {i}"), None, None);
    }
    let itens = p.historico_sql(&b);
    assert_eq!(itens.len(), 30);
    assert_eq!(MAX_COMANDOS, 30);
    assert_eq!(itens[0].sql, "SELECT 36");
    assert_eq!(itens[29].sql, "SELECT 7");
}

// Regra do Windows (caminhos `C:\…`, a mesma chave com outra caixa ou barra, o JSON gravado com `\\`):
// fora dele, os caminhos seguem o posixpath (`caminhos::testes::fora_do_windows_e_o_posixpath`).
#[test]
#[cfg(windows)]
fn historico_separado_por_banco_e_normalizado() {
    let tmp = PastaTemp::nova("separado");
    let p = abrir(&tmp.arquivo());
    let (vendas, estoque) = (tmp.caminho().join("Vendas.db"), tmp.caminho().join("estoque.db"));
    p.registrar_sql(&vendas, "SELECT * FROM clientes", None, None);
    p.registrar_sql(&estoque, "SELECT * FROM produtos", None, None);
    let outra_grafia = texto(&vendas).to_lowercase().replace('\\', "/");
    assert_eq!(sqls(&p.historico_sql(&outra_grafia)), ["SELECT * FROM clientes"]);
    assert_eq!(sqls(&p.historico_sql(&estoque)), ["SELECT * FROM produtos"]);
    assert!(p.historico_sql(tmp.caminho().join("outro.db")).is_empty());
}

#[test]
fn historico_ignora_vazio_e_gigante() {
    let tmp = PastaTemp::nova("vazio");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    let b = tmp.caminho().join("vendas.db");
    p.registrar_sql(&b, "   ", None, None);
    p.registrar_sql(&b, "", None, None); // vazio
    p.registrar_sql(&b, &"INSERT INTO t VALUES (1);".repeat(10_000), None, None);
    assert!(p.historico_sql(&b).is_empty());
    assert!(!arquivo.exists());
}

#[test]
fn historico_de_muitos_bancos_descarta_o_mais_antigo() {
    let tmp = PastaTemp::nova("muitos");
    let mut p = abrir(&tmp.arquivo());
    p.max_bancos_no_historico = 3;
    let b = |nome: &str| tmp.caminho().join(format!("{nome}.db"));
    for nome in ["a", "b", "c"] {
        p.registrar_sql(b(nome), "SELECT 1", None, None);
    }
    p.registrar_sql(b("a"), "SELECT 2", None, None); // "a" volta a ser o mais recente
    p.registrar_sql(b("d"), "SELECT 1", None, None);
    assert!(p.historico_sql(b("b")).is_empty());
    assert_eq!(p.historico_sql(b("a")).len(), 2);
    assert!(!p.historico_sql(b("c")).is_empty() && !p.historico_sql(b("d")).is_empty());
}

#[test]
fn historico_guarda_o_resultado_inclusive_o_erro() {
    let tmp = PastaTemp::nova("hist-resultado");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    let b = tmp.caminho().join("vendas.db");
    p.registrar_sql(&b, "SELECT * FROM clientes", Some(ResultadoSql::Linhas { quantidade: 9, mais: false }), Some(1));
    p.registrar_sql(&b, "UPDATE t SET a = 1", Some(ResultadoSql::Alteradas(3)), Some(1));
    p.registrar_sql(&b, "CREATE TABLE x (a)", Some(ResultadoSql::Executado), Some(1));
    p.registrar_sql(
        &b,
        "SELECT 1; SELECT 2; SELECT * FROM nada",
        Some(ResultadoSql::erro(3, "A tabela não existe: nada.")),
        Some(3),
    );
    p.registrar_sql(&b, "SELECT * FROM grande", Some(ResultadoSql::Linhas { quantidade: 500, mais: true }), Some(1));
    p.registrar_sql(&b, "SELECT 1; SELECT longo", Some(ResultadoSql::Interrompida { numero: 2 }), Some(2));

    // outra instância lê do arquivo o mesmo que esta guardou
    let itens = abrir(&arquivo).historico_sql(&b);
    let resultados: Vec<_> = itens.iter().map(|i| (i.resultado.clone(), i.comandos)).collect();
    assert_eq!(
        resultados,
        [
            (Some(ResultadoSql::Interrompida { numero: 2 }), Some(2)),
            (Some(ResultadoSql::Linhas { quantidade: 500, mais: true }), Some(1)),
            (Some(ResultadoSql::Erro { numero: 3, mensagem: "A tabela não existe: nada.".into() }), Some(3)),
            (Some(ResultadoSql::Executado), Some(1)),
            (Some(ResultadoSql::Alteradas(3)), Some(1)),
            (Some(ResultadoSql::Linhas { quantidade: 9, mais: false }), Some(1)),
        ]
    );
    let lido = json_do_arquivo(&arquivo);
    let gravados = lido["historico_sql"].as_object().unwrap().values().next().unwrap().clone();
    assert_eq!(gravados[0]["resultado"], serde_json::json!({"tipo": "interrompida", "comando": 2}));
    assert_eq!(
        gravados[2],
        serde_json::json!({
            "sql": "SELECT 1; SELECT 2; SELECT * FROM nada",
            "quando": gravados[2]["quando"],
            "resultado": {"tipo": "erro", "comando": 3, "mensagem": "A tabela não existe: nada."},
            "comandos": 3,
        })
    );
    assert_eq!(gravados[1]["resultado"], serde_json::json!({"tipo": "linhas", "quantidade": 500, "mais": true}));
    assert_eq!(gravados[5]["resultado"], serde_json::json!({"tipo": "linhas", "quantidade": 9}));

    // repetido sobe para o topo com o resultado novo: a consulta corrigida deixa de mostrar o erro
    p.registrar_sql(
        &b,
        "SELECT 1; SELECT 2; SELECT * FROM nada",
        Some(ResultadoSql::Linhas { quantidade: 0, mais: false }),
        Some(3),
    );
    let topo = &p.historico_sql(&b)[0];
    assert_eq!(topo.resultado, Some(ResultadoSql::Linhas { quantidade: 0, mais: false }));
    assert_eq!(p.historico_sql(&b).len(), 6);
}

#[test]
fn mensagem_de_erro_longa_e_cortada() {
    let longa = "x".repeat(MAX_CARACTERES_MENSAGEM * 2);
    let ResultadoSql::Erro { numero, mensagem } = ResultadoSql::erro(1, &longa) else { unreachable!() };
    assert_eq!(numero, 1);
    assert_eq!(mensagem.chars().count(), MAX_CARACTERES_MENSAGEM);
    assert!(mensagem.ends_with('…'));
    assert_eq!(ResultadoSql::erro(2, "  curta \n"), ResultadoSql::Erro { numero: 2, mensagem: "curta".into() });
}

/// O `preferencias.json` da 0.1.x: os itens do histórico só com `sql` e `quando` (e só os que deram certo). Abre
/// como antes, sem resultado; regravado, os itens antigos continuam sem as chaves novas.
#[test]
fn historico_da_0_1_abre_sem_resultado() {
    let tmp = PastaTemp::nova("hist-0-1");
    let arquivo = tmp.arquivo();
    let b = tmp.caminho().join("vendas.db");
    let antigo = serde_json::json!({
        "versao": 1,
        "tema": "dark",
        "exportacao": {"csv_separador": ";", "csv_decimal": ",", "csv_bom": true, "json_indentado": true},
        "ultimo_diretorio": null,
        "recentes": [{"caminho": texto(&b), "aberto_em": "2026-10-10T09:00:00"}],
        "historico_sql": {
            chave_caminho(&b): [
                {"sql": "SELECT * FROM clientes", "quando": "2026-10-10T09:05:00"},
                {"sql": "SELECT 1", "quando": null}
            ]
        }
    });
    gravar_texto(&arquivo, &serde_json::to_string_pretty(&antigo).unwrap());

    let p = abrir(&arquivo);
    assert_eq!(p.tema(), Tema::Escuro);
    assert_eq!(
        p.historico_sql(&b),
        [
            ComandoSql {
                sql: "SELECT * FROM clientes".into(),
                quando: Some("2026-10-10T09:05:00".into()),
                resultado: None,
                comandos: None,
            },
            ComandoSql { sql: "SELECT 1".into(), quando: None, resultado: None, comandos: None },
        ]
    );

    // uma execução nova entra com o resultado; os itens antigos são regravados como estavam
    p.registrar_sql(&b, "SELECT * FROM nada", Some(ResultadoSql::erro(1, "A tabela não existe: nada.")), Some(1));
    let lido = json_do_arquivo(&arquivo);
    let gravados = lido["historico_sql"][chave_caminho(&b)].clone();
    assert_eq!(gravados[0]["resultado"]["tipo"], "erro");
    assert_eq!(gravados[1], serde_json::json!({"sql": "SELECT * FROM clientes", "quando": "2026-10-10T09:05:00"}));
    assert_eq!(gravados[2], serde_json::json!({"sql": "SELECT 1", "quando": null}));
}

#[test]
fn resultado_estranho_no_historico_fica_de_fora() {
    let tmp = PastaTemp::nova("hist-estranho");
    let arquivo = tmp.arquivo();
    let b = tmp.caminho().join("vendas.db");
    let estranho = serde_json::json!({
        "historico_sql": {
            chave_caminho(&b): [
                {"sql": "SELECT 1", "quando": null, "resultado": {"tipo": "desconhecido"}, "comandos": -2},
                {"sql": "SELECT 2", "quando": null, "resultado": {"tipo": "linhas", "quantidade": -1}},
                {"sql": "SELECT 3", "quando": null, "resultado": {"tipo": "erro", "comando": 0}},
                {"sql": "SELECT 4", "quando": null, "resultado": "erro", "comandos": "3"},
                {"sql": "SELECT 5", "quando": null, "resultado": {"tipo": "erro", "comando": 2}, "comandos": 2}
            ]
        }
    });
    gravar_texto(&arquivo, &estranho.to_string());
    let itens = abrir(&arquivo).historico_sql(&b);
    assert_eq!(sqls(&itens), ["SELECT 1", "SELECT 2", "SELECT 3", "SELECT 4", "SELECT 5"], "o comando fica");
    assert!(itens[..4].iter().all(|i| i.resultado.is_none() && i.comandos.is_none()), "{itens:?}");
    // erro sem mensagem: vale, com a mensagem vazia
    assert_eq!(itens[4].resultado, Some(ResultadoSql::Erro { numero: 2, mensagem: String::new() }));
    assert_eq!(itens[4].comandos, Some(2));
}

#[test]
fn limpar_recentes_e_historico_grava_o_arquivo_sem_eles() {
    let tmp = PastaTemp::nova("limpar");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    let (a, b) = (tmp.caminho().join("a.db"), tmp.caminho().join("b.db"));
    p.adicionar_recente(&a);
    p.adicionar_recente(&b);
    p.registrar_sql(&a, "SELECT 1", None, None);
    p.registrar_sql(&b, "SELECT 2", None, None);
    p.atualizar(Mudanca::tema(Tema::Escuro)).unwrap();

    p.limpar_recentes();
    assert!(p.recentes().is_empty());
    assert_eq!(sqls(&p.historico_sql(&a)), ["SELECT 1"], "o histórico não é dos recentes");
    p.limpar_historico_sql();
    assert!(p.historico_sql(&a).is_empty() && p.historico_sql(&b).is_empty());

    // o que o arquivo guarda: sem recentes nem histórico, e o resto (o tema) como estava
    let lido = json_do_arquivo(&arquivo);
    assert_eq!(lido["recentes"], serde_json::json!([]));
    assert_eq!(lido["historico_sql"], serde_json::json!({}));
    assert_eq!(lido["tema"], "dark");
    assert_eq!(abrir(&arquivo).tema(), Tema::Escuro, "outra instância lê o mesmo");
}

#[test]
fn limpar_o_que_ja_esta_vazio_nao_grava_nada() {
    let tmp = PastaTemp::nova("limpar_vazio");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    p.limpar_recentes();
    p.limpar_historico_sql();
    assert!(!arquivo.exists(), "nada a limpar, nada a gravar");
}

// ---- atualizar e validação --------------------------------------------------------------------------

#[test]
fn atualizar_tema_e_exportacao() {
    let tmp = PastaTemp::nova("atualizar");
    let p = abrir(&tmp.arquivo());
    p.atualizar(Mudanca::tema(Tema::Claro)).unwrap();
    p.atualizar(Mudanca::exportacao(MudancaExportacao {
        csv_decimal: Some('.'),
        json_indentado: Some(false),
        cabecalho: Some(false),
        ..Default::default()
    }))
    .unwrap();
    assert_eq!(p.tema(), Tema::Claro);
    assert_eq!(
        p.exportacao(),
        OpcoesExportacao {
            csv_separador: ';',
            csv_decimal: '.',
            json_indentado: false,
            cabecalho: Some(false),
            csv_proteger_formulas: None
        }
    );
    p.atualizar(Mudanca::default()).unwrap(); // None = não mexe
    assert_eq!(p.tema(), Tema::Claro);
    p.atualizar(Mudanca::tema(Tema::Escuro)).unwrap();
    assert_eq!(p.resumo().tema, Tema::Escuro);
}

#[test]
fn atualizar_recusa_valor_invalido() {
    type Caso = fn(&Preferencias) -> Resultado<()>;
    let casos: [(Caso, &str); 4] = [
        (
            |p| {
                let tema: Tema = "roxo".parse()?;
                p.atualizar(Mudanca::tema(tema))
            },
            "Tema desconhecido: 'roxo'. Use system, light ou dark.",
        ),
        (
            |p| p.atualizar(Mudanca::exportacao(MudancaExportacao { csv_separador: Some('|'), ..Default::default() })),
            "O separador do CSV precisa ser ponto e vírgula, vírgula ou tabulação.",
        ),
        (
            |p| p.atualizar(Mudanca::exportacao(MudancaExportacao { csv_decimal: Some(';'), ..Default::default() })),
            "O separador decimal precisa ser vírgula ou ponto.",
        ),
        (
            |p| p.atualizar(Mudanca::ultimo_diretorio(Some(PathBuf::from("  ")))),
            "A última pasta precisa ser um caminho.",
        ),
    ];
    for (caso, mensagem) in casos {
        let tmp = PastaTemp::nova("invalido");
        let arquivo = tmp.arquivo();
        let p = abrir(&arquivo);
        let erro = caso(&p).unwrap_err();
        assert_eq!(erro.status, 400);
        assert_eq!(erro.mensagem, mensagem);
        assert!(erro.mensagem.ends_with('.'));
        assert!(p.tema() == Tema::Sistema && p.exportacao() == OpcoesExportacao::default());
        assert!(!arquivo.exists());
    }
    let erro = abrir(Path::new("x"))
        .atualizar(Mudanca::exportacao(MudancaExportacao { csv_separador: Some('|'), ..Default::default() }))
        .unwrap_err();
    assert_eq!(erro.detalhe.as_deref(), Some("csv_separador='|'"));
}

#[test]
fn ultimo_diretorio() {
    let tmp = PastaTemp::nova("ultimo");
    let p = abrir(&tmp.arquivo());
    let dados = tmp.caminho().join("dados");
    let b = banco(&dados, "vendas.db", b"x");
    p.atualizar(Mudanca::ultimo_diretorio(Some(dados.clone()))).unwrap();
    assert_eq!(p.ultimo_diretorio(), Some(texto(&dados)));
    p.atualizar(Mudanca::ultimo_diretorio(Some(b))).unwrap(); // arquivo → a pasta dele
    assert_eq!(p.ultimo_diretorio(), Some(texto(&dados)));
    p.atualizar(Mudanca::ultimo_diretorio(None)).unwrap();
    assert_eq!(p.ultimo_diretorio(), None);
}

// ---- arquivo corrompido e saneamento ----------------------------------------------------------------

#[test]
fn arquivo_corrompido_e_guardado_a_parte_e_recomeca_do_padrao() {
    let tmp = PastaTemp::nova("corrompido");
    let arquivo = tmp.arquivo();
    fs::create_dir_all(arquivo.parent().unwrap()).unwrap();
    fs::write(&arquivo, r#"{"tema": "dark", "recentes": ["#).unwrap();
    let p = abrir(&arquivo);
    assert_eq!(p.tema(), Tema::Sistema);
    assert!(p.recentes().is_empty());
    let corrompido = arquivo.with_file_name("preferencias.json.corrompido");
    assert_eq!(fs::read_to_string(&corrompido).unwrap(), r#"{"tema": "dark", "recentes": ["#);
    assert!(!arquivo.exists());

    p.adicionar_recente(tmp.caminho().join("novo.db"));
    assert!(json_do_arquivo(&arquivo)["recentes"][0]["caminho"].as_str().unwrap().ends_with("novo.db"));
}

#[test]
fn outros_conteudos_ilegiveis_tambem_recomecam() {
    let conteudos: [&[u8]; 4] = [b"[1, 2, 3]", b"\xff\xfe\x00lixo", b"", b"null"];
    for conteudo in conteudos {
        let tmp = PastaTemp::nova("ilegivel");
        let arquivo = tmp.arquivo();
        fs::create_dir_all(arquivo.parent().unwrap()).unwrap();
        fs::write(&arquivo, conteudo).unwrap();
        assert_eq!(abrir(&arquivo).resumo().tema, Tema::Sistema);
        assert_eq!(fs::read(arquivo.with_file_name("preferencias.json.corrompido")).unwrap(), conteudo);
    }
}

// Regra do Windows (caminhos `C:\…`, a mesma chave com outra caixa ou barra, o JSON gravado com `\\`):
// fora dele, os caminhos seguem o posixpath (`caminhos::testes::fora_do_windows_e_o_posixpath`).
#[test]
#[cfg(windows)]
fn arquivo_tolerado_nao_vira_corrompido() {
    // 200 níveis de aninhamento (até numa chave desconhecida), NaN, Infinity, número fora da faixa e surrogate solto
    // são lidos. Separar como corrompido apagaria tema, recentes e histórico.
    let tmp = PastaTemp::nova("fundo");
    let arquivo = tmp.arquivo();
    fs::create_dir_all(arquivo.parent().unwrap()).unwrap();
    let fundo = format!("{}{}", "[".repeat(200), "]".repeat(200));
    let conteudo = format!(
        r#"{{"tema": "dark", "recentes": [{{"caminho": "C:\\x.db", "aberto_em": NaN}}], "x": {fundo},
  "y": [NaN, Infinity, -Infinity, 1e400, -1e400, 1e-400, 123456789012345678901234567890],
  "historico_sql": {{"c:\\x.db": [{{"sql": "SELECT 1", "quando": "\udc00"}}]}}, "z": "\ud800\ud83d\ude00"}}"#
    );
    fs::write(&arquivo, &conteudo).unwrap();
    let p = abrir(&arquivo);
    assert_eq!(p.tema(), Tema::Escuro);
    let recentes = p.recentes();
    assert_eq!(recentes.iter().map(|r| r.caminho.as_str()).collect::<Vec<_>>(), [r"C:\x.db"]);
    assert_eq!(recentes[0].aberto_em, None);
    // uma String do Rust não representa o surrogate solto: fica U+FFFD
    assert_eq!(p.historico_sql(r"C:\X.db")[0].quando.as_deref(), Some("\u{fffd}"));
    assert!(!arquivo.with_file_name("preferencias.json.corrompido").exists());
    assert_eq!(fs::read_to_string(&arquivo).unwrap(), conteudo); // só leu

    // a mudança seguinte grava o que foi aproveitado, e as chaves que esta versão não conhece
    // (lido pelo leitor do crate: o "y" regravado tem NaN, que o serde_json recusa)
    p.adicionar_recente(r"C:\y.db");
    let gravado = json_lido(&arquivo);
    assert_eq!(gravado.campo("tema"), Some(&texto_json("dark")));
    assert_eq!(itens(gravado.campo("recentes"))[1].campo("caminho"), Some(&texto_json(r"C:\x.db")));
    let historico = itens(gravado.campo("historico_sql").and_then(|h| h.campo(r"c:\x.db")));
    assert_eq!(historico[0].campo("sql"), Some(&texto_json("SELECT 1")));
    // "y" e "z" vão depois das conhecidas; "x", com 200 níveis, passa do fundo que o leitor guarda e fica de fora
    assert_eq!(
        chaves(&gravado),
        ["versao", "tema", "exportacao", "ultimo_diretorio", "recentes", "historico_sql", "y", "z"]
    );
    let y = itens(gravado.campo("y"));
    assert!(matches!(y[0], Json::Real(r) if r.is_nan()));
    let (inf, menos_inf) = (f64::INFINITY, f64::NEG_INFINITY);
    assert_eq!(y[1..6], [inf, menos_inf, inf, menos_inf, 0.0].map(Json::Real));
    assert_eq!(gravado.campo("z"), Some(&texto_json("\u{fffd}\u{1f600}")));
}

// Regra do Windows (caminhos `C:\…`, a mesma chave com outra caixa ou barra, o JSON gravado com `\\`):
// fora dele, os caminhos seguem o posixpath (`caminhos::testes::fora_do_windows_e_o_posixpath`).
#[test]
#[cfg(windows)]
fn campos_estranhos_voltam_ao_padrao_sem_perder_o_resto() {
    let tmp = PastaTemp::nova("estranhos");
    let arquivo = tmp.arquivo();
    fs::create_dir_all(arquivo.parent().unwrap()).unwrap();
    let bom = texto(&tmp.caminho().join("bom.db"));
    let conteudo = serde_json::json!({
        "tema": "roxo",
        "exportacao": {"csv_separador": ",", "csv_decimal": "x", "json_indentado": "sim"},
        "ultimo_diretorio": 42,
        "recentes": [{"caminho": bom, "aberto_em": "2026-09-20T09:00:00"}, {"caminho": bom.to_uppercase()},
                     "lixo", {"sem": "caminho"}, {"caminho": ""}],
        "historico_sql": {"x": [{"sql": "SELECT 1", "quando": "2026-09-20T09:00:00"}, {"sql": 5}], "y": "nada"},
    });
    fs::write(&arquivo, conteudo.to_string()).unwrap();
    let p = abrir(&arquivo);
    assert_eq!(p.tema(), Tema::Sistema);
    assert_eq!(p.exportacao(), OpcoesExportacao { csv_separador: ',', ..Default::default() });
    assert_eq!(p.ultimo_diretorio(), None);
    let itens = p.recentes();
    assert_eq!(itens.iter().map(|i| i.caminho.as_str()).collect::<Vec<_>>(), [bom.as_str()]);
    assert_eq!(itens[0].aberto_em.as_deref(), Some("2026-09-20T09:00:00"));
    assert!(!arquivo.with_file_name("preferencias.json.corrompido").exists());
}

#[test]
fn aceita_arquivo_com_bom_do_bloco_de_notas() {
    let tmp = PastaTemp::nova("bom");
    let arquivo = tmp.arquivo();
    fs::create_dir_all(arquivo.parent().unwrap()).unwrap();
    fs::write(&arquivo, b"\xEF\xBB\xBF{\"tema\": \"dark\"}").unwrap();
    assert_eq!(abrir(&arquivo).tema(), Tema::Escuro);
}

#[test]
fn mudanca_feita_por_fora_e_relida() {
    let tmp = PastaTemp::nova("por-fora");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    p.atualizar(Mudanca::tema(Tema::Claro)).unwrap();
    let mut dados = json_do_arquivo(&arquivo);
    dados["tema"] = "dark".into();
    // tamanho diferente: muda a assinatura
    fs::write(&arquivo, format!("{}\n\n", serde_json::to_string(&dados).unwrap())).unwrap();
    assert_eq!(p.tema(), Tema::Escuro);
}

// ---- gravação atômica -------------------------------------------------------------------------------

/// O que sobra na pasta além do arquivo e do `.lock` (a trava entre instâncias fica, vazia, de propósito).
fn sobras(pasta: &Path) -> Vec<String> {
    let mut nomes: Vec<String> = fs::read_dir(pasta)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n != "preferencias.json" && n != "preferencias.json.lock")
        .collect();
    nomes.sort();
    nomes
}

#[test]
fn gravacao_nao_deixa_temporario() {
    let tmp = PastaTemp::nova("temporario");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    for i in 0..5 {
        p.adicionar_recente(tmp.caminho().join(format!("b{i}.db")));
    }
    assert!(sobras(arquivo.parent().unwrap()).is_empty());
    let conteudo = fs::read_to_string(&arquivo).unwrap();
    assert_eq!(serde_json::from_str::<serde_json::Value>(&conteudo).unwrap()["versao"], 1);
    assert!(!conteudo.contains("\r\n"));
}

/// Disco que recusa a troca quando ligado.
struct RecusarTroca(AtomicBool);

impl Disco for RecusarTroca {
    fn trocar(&self, origem: &Path, destino: &Path) -> io::Result<()> {
        if self.0.load(Ordering::SeqCst) {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Acesso negado"));
        }
        fs::rename(origem, destino)
    }
}

#[test]
fn falha_na_troca_preserva_o_arquivo_anterior() {
    let tmp = PastaTemp::nova("falha-troca");
    let arquivo = tmp.arquivo();
    let disco = Arc::new(RecusarTroca(AtomicBool::new(false)));
    let p = Preferencias::com_disco(arquivo.clone(), disco.clone());
    p.atualizar(Mudanca::tema(Tema::Escuro)).unwrap();
    let antes = fs::read(&arquivo).unwrap();

    disco.0.store(true, Ordering::SeqCst);
    p.atualizar(Mudanca::tema(Tema::Claro)).unwrap(); // preferência não gravada não derruba o aplicativo

    assert_eq!(fs::read(&arquivo).unwrap(), antes);
    assert!(sobras(arquivo.parent().unwrap()).is_empty());
    assert_eq!(p.tema(), Tema::Claro); // vale em memória até fechar
}

/// Disco que escreve metade e falha quando ligado.
struct EscritaPelaMetade(AtomicBool);

impl Disco for EscritaPelaMetade {
    fn escrever(&self, arquivo: &mut fs::File, bytes: &[u8]) -> io::Result<()> {
        if self.0.load(Ordering::SeqCst) {
            arquivo.write_all(br#"{"tema": "li"#)?;
            return Err(io::Error::other("Disco cheio"));
        }
        arquivo.write_all(bytes)
    }
}

#[test]
fn falha_no_meio_da_escrita_preserva_o_arquivo_anterior() {
    let tmp = PastaTemp::nova("falha-escrita");
    let arquivo = tmp.arquivo();
    let disco = Arc::new(EscritaPelaMetade(AtomicBool::new(false)));
    let p = Preferencias::com_disco(arquivo.clone(), disco.clone());
    p.atualizar(Mudanca::tema(Tema::Escuro)).unwrap();
    let antes = fs::read(&arquivo).unwrap();

    disco.0.store(true, Ordering::SeqCst);
    p.atualizar(Mudanca::tema(Tema::Claro)).unwrap();
    assert_eq!(fs::read(&arquivo).unwrap(), antes);
    assert!(sobras(arquivo.parent().unwrap()).is_empty());
    assert_eq!(abrir(&arquivo).tema(), Tema::Escuro);
}

/// Disco cuja troca acha o destino em uso nas duas primeiras vezes.
struct OcupadoDuasVezes(AtomicUsize);

impl Disco for OcupadoDuasVezes {
    fn trocar(&self, origem: &Path, destino: &Path) -> io::Result<()> {
        if self.0.fetch_add(1, Ordering::SeqCst) < 2 {
            return Err(io::Error::from_raw_os_error(32)); // ERROR_SHARING_VIOLATION
        }
        fs::rename(origem, destino)
    }
}

#[test]
fn troca_tenta_de_novo_quando_o_destino_esta_ocupado() {
    let tmp = PastaTemp::nova("tenta-de-novo");
    let arquivo = tmp.arquivo();
    let disco = Arc::new(OcupadoDuasVezes(AtomicUsize::new(0)));
    let p = Preferencias::com_disco(arquivo.clone(), disco.clone());
    p.atualizar(Mudanca::tema(Tema::Escuro)).unwrap();
    assert_eq!(disco.0.load(Ordering::SeqCst), 3);
    assert_eq!(abrir(&arquivo).tema(), Tema::Escuro);
}

// ---- duas instâncias (dois processos) ao mesmo tempo ------------------------------------------------

fn agora_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

/// Processo filho de [`duas_instancias_gravando_juntas_nao_perdem_mudancas`]: o mesmo executável de testes,
/// rodando só este teste, com o arquivo, o nome e a hora de largada no ambiente. Sem o ambiente, não faz nada.
#[test]
#[ignore = "processo filho dos testes de duas instâncias"]
fn filho_prefs() {
    let (Ok(arquivo), Ok(quem), Ok(largada)) =
        (std::env::var("PREFS_FILHO_ARQUIVO"), std::env::var("PREFS_FILHO_QUEM"), std::env::var("PREFS_FILHO_LARGADA"))
    else {
        return;
    };
    let largada: u128 = largada.parse().unwrap();
    let p = Preferencias::abrir(Some(PathBuf::from(arquivo)));
    let _ = p.tema(); // lê uma vez, como o aplicativo faz ao subir
    while agora_ms() < largada {
        thread::sleep(Duration::from_millis(1));
    }
    for i in 0..20 {
        p.registrar_sql(format!("C:/bancos/{quem}.db"), &format!("SELECT {i} -- {quem}"), None, None);
        p.adicionar_recente(format!("C:/bancos/{quem}_{i}.db"));
    }
}

/// Inicia o [`filho_prefs`] num processo à parte.
#[cfg(windows)]
pub(crate) fn processo_filho(arquivo: &Path, quem: &str, largada_ms: u128) -> std::process::Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["testes::filho_prefs", "--exact", "--ignored", "--test-threads=1", "--quiet"])
        .env("PREFS_FILHO_ARQUIVO", arquivo)
        .env("PREFS_FILHO_QUEM", quem)
        .env("PREFS_FILHO_LARGADA", largada_ms.to_string())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap()
}

// Regra do Windows (caminhos `C:\…`, a mesma chave com outra caixa ou barra, o JSON gravado com `\\`):
// fora dele, os caminhos seguem o posixpath (`caminhos::testes::fora_do_windows_e_o_posixpath`).
#[test]
#[cfg(windows)]
fn duas_instancias_gravando_juntas_nao_perdem_mudancas() {
    // Antes, a gravação de uma instância entre a leitura e a gravação da outra sumia (sem trava entre processos).
    // E, com a trava, uma instância às vezes fazia as 40 mudanças antes de a outra conseguir a primeira (sem a vez de
    // trava.rs): falhava 1 em cada 3 vezes, com os 15 recentes todos de uma só.
    let tmp = PastaTemp::nova("dois-processos");
    let arquivo = tmp.arquivo();
    let largada = agora_ms() + 1500;
    let filhos: Vec<_> = ["A", "B"].iter().map(|quem| processo_filho(&arquivo, quem, largada)).collect();
    for mut filho in filhos {
        assert!(filho.wait().unwrap().success());
    }
    let p = abrir(&arquivo);
    assert_eq!(p.historico_sql("C:/bancos/A.db").len(), 20);
    assert_eq!(p.historico_sql("C:/bancos/B.db").len(), 20);
    let dados = json_do_arquivo(&arquivo);
    let recentes: Vec<&str> =
        dados["recentes"].as_array().unwrap().iter().map(|r| r["caminho"].as_str().unwrap()).collect();
    // os 15 mais recentes são os últimos de cada uma, intercalados: nenhuma apagou a outra por completo
    conferir_recentes_intercalados(&recentes, &["A", "B"], true);
}

/// Os 15 recentes são os últimos de cada instância, intercalados e sem buraco: os de cada uma formam o fim da
/// sequência dela (…, 18, 19). Uma mudança perdida (gravada por cima pela outra) deixaria um buraco.
///
/// `exigir_o_19_de_todos`: o 19 de cada instância está lá. Vale para instâncias que andam no mesmo passo; com
/// false, basta um 19 (uma pode terminar bem antes e as gravações da outra ocupam os 15).
#[cfg(windows)]
pub(crate) fn conferir_recentes_intercalados(recentes: &[&str], quem: &[&str], exigir_o_19_de_todos: bool) {
    assert_eq!(recentes.len(), MAX_RECENTES, "{recentes:?}");
    let mut algum_19 = false;
    for q in quem {
        let prefixo = format!("C:\\bancos\\{q}_");
        let mut indices: Vec<u32> = recentes
            .iter()
            .filter_map(|r| r.strip_prefix(prefixo.as_str())?.strip_suffix(".db")?.parse().ok())
            .collect();
        indices.sort_unstable();
        match indices.first() {
            Some(&primeiro) => {
                assert_eq!(indices, (primeiro..20).collect::<Vec<_>>(), "buraco nos recentes de {q}: {recentes:?}");
                algum_19 = true;
            }
            None => assert!(!exigir_o_19_de_todos, "nenhum recente de {q} (falta o {q}_19): {recentes:?}"),
        }
    }
    assert!(algum_19, "{recentes:?}");
}

/// Disco cuja leitura do arquivo de preferências falha (em uso) quando ligado.
#[cfg(windows)]
struct LeituraOcupada {
    ocupado: AtomicBool,
    alvo: PathBuf,
}

#[cfg(windows)]
impl Disco for LeituraOcupada {
    fn ler(&self, caminho: &Path) -> io::Result<Vec<u8>> {
        if self.ocupado.load(Ordering::SeqCst) && caminho == self.alvo {
            return Err(io::Error::from_raw_os_error(32));
        }
        fs::read(caminho)
    }
}

// Regra do Windows (caminhos `C:\…`, a mesma chave com outra caixa ou barra, o JSON gravado com `\\`):
// fora dele, os caminhos seguem o posixpath (`caminhos::testes::fora_do_windows_e_o_posixpath`).
#[test]
#[cfg(windows)]
fn arquivo_ilegivel_na_hora_de_mudar_nao_grava_por_cima() {
    // Arquivo ocupado ao reler: gravar agora apagaria o que a outra instância gravou; a mudança fica em memória.
    let tmp = PastaTemp::nova("ilegivel-na-hora");
    let arquivo = tmp.arquivo();
    let disco = Arc::new(LeituraOcupada { ocupado: AtomicBool::new(false), alvo: arquivo.clone() });
    let p = Preferencias::com_disco(arquivo.clone(), disco.clone());
    p.adicionar_recente(r"C:\bancos\a.db");
    let outra = abrir(&arquivo);
    outra.adicionar_recente(r"C:\bancos\da_outra.db"); // a outra instância gravou

    disco.ocupado.store(true, Ordering::SeqCst);
    p.atualizar(Mudanca::tema(Tema::Escuro)).unwrap();
    disco.ocupado.store(false, Ordering::SeqCst);
    let gravado = json_do_arquivo(&arquivo);
    assert_eq!(gravado["recentes"][0]["caminho"], r"C:\bancos\da_outra.db");
    assert_eq!(gravado["tema"], "system");
}

// ---- do porte ---------------------------------------------------------------------------------------

/// Um arquivo sem chaves extras (recuo de 2 espaços, sem escapar o que não é ASCII, a partir do padrão
/// modificado), com `<DEL>` e `<LS>` no lugar de U+007F e U+2028.
const GRAVADO_SEM_EXTRAS: &str = r#"{
  "versao": 1,
  "tema": "dark",
  "exportacao": {
    "csv_separador": "\t",
    "csv_decimal": ",",
    "json_indentado": true,
    "csv_proteger_formulas": false,
    "cabecalho": true
  },
  "ultimo_diretorio": "C:\\Dados\\Ação",
  "recentes": [
    {
      "caminho": "C:\\Dados\\Ação\\vendas 'x'.db",
      "aberto_em": "2026-10-07T09:08:07"
    },
    {
      "caminho": "\\\\srv\\share\\b.db",
      "aberto_em": null
    }
  ],
  "historico_sql": {
    "c:\\dados\\ação\\vendas 'x'.db": [
      {
        "sql": "SELECT \"a\tb\", 'ç' -- \u0001<DEL><LS>\n  FROM t",
        "quando": "2026-10-07T09:08:07"
      },
      {
        "sql": "SELECT 1",
        "quando": null
      }
    ],
    "c:\\b.db": [
      {
        "sql": "SELECT 2",
        "quando": "2026-01-02T03:04:05"
      }
    ]
  }
}"#;

#[test]
fn formato_sem_extras_volta_igual() {
    let original = GRAVADO_SEM_EXTRAS.replace("<DEL>", "\u{7f}").replace("<LS>", "\u{2028}");
    let dados = sanear(&json::ler(original.as_bytes()).unwrap(), MAX_BANCOS_NO_HISTORICO).unwrap();
    assert_eq!(dados.tema, Tema::Escuro);
    assert_eq!(
        dados.exportacao.opcoes,
        OpcoesExportacao {
            csv_separador: '\t',
            csv_decimal: ',',
            json_indentado: true,
            cabecalho: Some(true),
            csv_proteger_formulas: Some(false)
        }
    );
    assert_eq!(dados.historico_sql[0].1[0].sql, "SELECT \"a\tb\", 'ç' -- \u{1}\u{7f}\u{2028}\n  FROM t");
    assert_eq!(dados.serializar(), original);
}

#[test]
fn padrao_gravado_no_formato() {
    let esperado = "{\n  \"versao\": 1,\n  \"tema\": \"system\",\n  \"exportacao\": {\n    \"csv_separador\": \";\",\n    \
                    \"csv_decimal\": \",\",\n    \"json_indentado\": true\n  },\n  \"ultimo_diretorio\": null,\n  \
                    \"recentes\": [],\n  \"historico_sql\": {}\n}";
    assert_eq!(Dados::padrao().serializar(), esperado);
}

#[test]
fn tema_como_str_e_de_texto() {
    for (tema, texto) in [(Tema::Sistema, "system"), (Tema::Claro, "light"), (Tema::Escuro, "dark")] {
        assert_eq!(tema.como_str(), texto);
        assert_eq!(Tema::de_texto(texto).unwrap(), tema);
        assert_eq!(tema.to_string(), texto);
    }
    assert_eq!(Tema::default(), Tema::Sistema);
    assert_eq!(Tema::de_texto("Dark").unwrap_err().mensagem, "Tema desconhecido: 'Dark'. Use system, light ou dark.");
}

#[test]
fn preferencias_e_send_sync() {
    fn exige<T: Send + Sync>() {}
    exige::<Preferencias>();
    exige::<Arc<Preferencias>>();
}

#[test]
fn trava_exclui_outra_instancia_do_mesmo_processo() {
    let tmp = PastaTemp::nova("trava");
    let caminho = tmp.caminho().join("preferencias.json.lock");
    let primeira = TravaEntreProcessos::adquirir(&caminho, Duration::from_secs(1));
    assert!(primeira.travado());
    let inicio = Instant::now();
    let segunda = TravaEntreProcessos::adquirir(&caminho, Duration::from_millis(200));
    assert!(!segunda.travado()); // segue sem ela depois do prazo
    assert!(inicio.elapsed() >= Duration::from_millis(200));
    drop(segunda);
    drop(primeira);
    assert!(TravaEntreProcessos::adquirir(&caminho, Duration::from_millis(200)).travado());
}

#[test]
fn quem_espera_a_trava_bate_e_quem_solta_cede_a_vez() {
    // Antes, quem gravava várias mudanças seguidas soltava e travava de novo em microssegundos, e a outra
    // instância (que tenta de 10 em 10 ms) ficava de fora até o fim da rajada.
    let tmp = PastaTemp::nova("vez");
    let caminho = tmp.caminho().join("preferencias.json.lock");
    let sozinha = TravaEntreProcessos::adquirir(&caminho, Duration::from_secs(1));
    assert!(sozinha.travado());
    assert!(!sozinha.soltar(), "ninguém esperou: nada de cortesia");

    let primeira = TravaEntreProcessos::adquirir(&caminho, Duration::from_secs(1));
    assert!(primeira.travado());
    let outro = caminho.clone();
    let esperando = thread::spawn(move || TravaEntreProcessos::adquirir(&outro, Duration::from_millis(100)).travado());
    assert!(!esperando.join().unwrap());
    assert!(primeira.soltar(), "a outra bateu enquanto a trava estava presa");
}

// Regra do Windows (caminhos `C:\…`, a mesma chave com outra caixa ou barra, o JSON gravado com `\\`):
// fora dele, os caminhos seguem o posixpath (`caminhos::testes::fora_do_windows_e_o_posixpath`).
#[test]
#[cfg(windows)]
fn mudanca_espera_a_trava_de_outra_instancia() {
    let tmp = PastaTemp::nova("espera-trava");
    let arquivo = tmp.arquivo();
    let p = Arc::new(abrir(&arquivo));
    let trava = TravaEntreProcessos::adquirir(&p.arquivo_trava(), Duration::from_secs(1));
    assert!(trava.travado());
    let outra = Arc::clone(&p);
    let inicio = Instant::now();
    let gravando = thread::spawn(move || outra.adicionar_recente(r"C:\bancos\x.db"));
    thread::sleep(Duration::from_millis(500));
    assert!(!arquivo.exists()); // ainda esperando a trava
    drop(trava);
    gravando.join().unwrap();
    assert!(inicio.elapsed() >= Duration::from_millis(500));
    assert_eq!(p.recentes()[0].caminho, r"C:\bancos\x.db");
}

#[test]
fn mtime_arredonda_ao_microssegundo() {
    let local = |s: i64| chrono::Local.timestamp_opt(s, 0).unwrap().format("%Y-%m-%dT%H:%M:%S").to_string();
    let t = |nanos: u64| UNIX_EPOCH + Duration::from_nanos(nanos);
    assert_eq!(iso_de_mtime(t(1_700_000_000_000_000_000)).unwrap(), local(1_700_000_000));
    assert_eq!(iso_de_mtime(t(1_700_000_000_900_000_000)).unwrap(), local(1_700_000_000));
    // 100.9999996 s vira 101 s; 100.9999994 s fica em 100 s
    assert_eq!(iso_de_mtime(t(100_999_999_600)).unwrap(), local(101));
    assert_eq!(iso_de_mtime(t(100_999_999_400)).unwrap(), local(100));
}

#[test]
fn exportacao_lembra_a_ordem_das_opcoes_opcionais() {
    let mut e = Exportacao::default();
    e.aplicar(&MudancaExportacao { csv_proteger_formulas: Some(true), ..Default::default() });
    e.aplicar(&MudancaExportacao { cabecalho: Some(false), csv_proteger_formulas: Some(false), ..Default::default() });
    let chaves: Vec<String> = match e.para_json() {
        Json::Objeto(pares) => pares.into_iter().map(|(k, _)| k).collect(),
        _ => unreachable!(),
    };
    assert_eq!(chaves, ["csv_separador", "csv_decimal", "json_indentado", "csv_proteger_formulas", "cabecalho"]);
    let todas = MudancaExportacao::from(OpcoesExportacao::default());
    assert_eq!(todas.csv_separador, Some(';'));
    assert_eq!(todas.cabecalho, None);
}

// ---- chaves que esta versão não conhece -------------------------------------------------------------

/// O arquivo de uma versão mais nova do DataBox: chaves desconhecidas no arquivo (antes e depois das conhecidas)
/// e nos recentes, entre elas o "tipo".
const GRAVADO_POR_UMA_VERSAO_NOVA: &str = r#"{
  "paineis": {"explorador": true, "largura": 258},
  "versao": 1,
  "tema": "dark",
  "recentes": [
    {"tipo": "banco", "caminho": "C:\\Dados\\vendas.db", "aberto_em": "2026-10-07T09:08:07", "fixado": true},
    {"caminho": "C:\\Projetos", "tipo": "pasta", "aberto_em": null, "abertas": ["a.db", "b.db"]}
  ],
  "historico_sql": {},
  "pastas": [{"caminho": "C:\\Projetos", "nome": "Projetos"}]
}"#;

/// O [`GRAVADO_POR_UMA_VERSAO_NOVA`] regravado por esta versão com o tema claro: as chaves desconhecidas depois
/// das conhecidas, na ordem lida.
#[cfg(windows)]
const REGRAVADO_COM_O_TEMA_CLARO: &str = r#"{
  "versao": 1,
  "tema": "light",
  "exportacao": {
    "csv_separador": ";",
    "csv_decimal": ",",
    "json_indentado": true
  },
  "ultimo_diretorio": null,
  "recentes": [
    {
      "caminho": "C:\\Dados\\vendas.db",
      "aberto_em": "2026-10-07T09:08:07",
      "tipo": "banco",
      "fixado": true
    },
    {
      "caminho": "C:\\Projetos",
      "aberto_em": null,
      "tipo": "pasta",
      "abertas": [
        "a.db",
        "b.db"
      ]
    }
  ],
  "historico_sql": {},
  "paineis": {
    "explorador": true,
    "largura": 258
  },
  "pastas": [
    {
      "caminho": "C:\\Projetos",
      "nome": "Projetos"
    }
  ]
}"#;

/// `[[[…]]]` com `n` níveis.
#[cfg(windows)]
fn listas(n: usize) -> String {
    format!("{}{}", "[".repeat(n), "]".repeat(n))
}

// Regra do Windows (caminhos `C:\…`, a mesma chave com outra caixa ou barra, o JSON gravado com `\\`):
// fora dele, os caminhos seguem o posixpath (`caminhos::testes::fora_do_windows_e_o_posixpath`).
#[test]
#[cfg(windows)]
fn chaves_desconhecidas_sobrevivem_a_gravar() {
    let tmp = PastaTemp::nova("desconhecidas");
    let arquivo = tmp.arquivo();
    gravar_texto(&arquivo, GRAVADO_POR_UMA_VERSAO_NOVA);
    let p = abrir(&arquivo);
    assert_eq!(p.tema(), Tema::Escuro);
    p.atualizar(Mudanca::tema(Tema::Claro)).unwrap();
    assert_eq!(fs::read_to_string(&arquivo).unwrap(), REGRAVADO_COM_O_TEMA_CLARO);

    // outra instância lê e regrava sem perder nada nem mudar a ordem
    abrir(&arquivo).registrar_sql(r"C:\Dados\vendas.db", "SELECT 1", None, None);
    let gravado = json_lido(&arquivo);
    let regravado = json::ler(REGRAVADO_COM_O_TEMA_CLARO.as_bytes()).unwrap();
    assert_eq!(chaves(&gravado), chaves(&regravado));
    for chave in ["tema", "recentes", "paineis", "pastas"] {
        assert_eq!(gravado.campo(chave), regravado.campo(chave), "{chave}");
    }
    assert_eq!(itens(gravado.campo("historico_sql").and_then(|h| h.campo(r"c:\dados\vendas.db"))).len(), 1);

    // um valor que chega ao fundo que o leitor guarda (64 níveis, contando o objeto de fora) fica de fora:
    // regravado, sairia cortado. Num recente, o objeto dele já é o 3º nível.
    let fundo = format!(
        r#"{{"raso": {}, "fundo": {}, "recentes": [{{"caminho": "C:\\x.db", "raso": {}, "fundo": {}}}]}}"#,
        listas(62),
        listas(63),
        listas(60),
        listas(61)
    );
    let dados = sanear(&json::ler(fundo.as_bytes()).unwrap(), MAX_BANCOS_NO_HISTORICO).unwrap();
    assert_eq!(dados.extras, [("raso".to_string(), json::ler(listas(62).as_bytes()).unwrap())]);
    assert_eq!(dados.recentes[0].extras, [("raso".to_string(), json::ler(listas(60).as_bytes()).unwrap())]);
}

// Regra do Windows (caminhos `C:\…`, a mesma chave com outra caixa ou barra, o JSON gravado com `\\`):
// fora dele, os caminhos seguem o posixpath (`caminhos::testes::fora_do_windows_e_o_posixpath`).
#[test]
#[cfg(windows)]
fn adicionar_recente_leva_os_extras() {
    let tmp = PastaTemp::nova("leva-extras");
    let arquivo = tmp.arquivo();
    gravar_texto(
        &arquivo,
        r#"{"recentes": [
            {"caminho": "C:\\Dados\\Vendas.db", "aberto_em": "2026-10-01T08:00:00", "fixado": true, "cor": "azul"},
            {"caminho": "C:\\Dados\\estoque.db", "tipo": "banco", "fixado": false},
            {"caminho": "C:\\Dados\\virou_banco.db", "tipo": "pasta", "abertas": ["x"]}
        ]}"#,
    );
    let p = abrir(&arquivo);
    p.adicionar_recente("c:/dados/VENDAS.DB"); // a mesma chave de caminho, com outra grafia
    p.adicionar_recente(r"C:\Dados\estoque.db");
    p.adicionar_recente(r"C:\Dados\novo.db");
    p.adicionar_recente(r"C:\Dados\virou_banco.db"); // era uma pasta; agora foi aberto como banco
    assert_eq!(nomes(&p.recentes()), ["virou_banco.db", "novo.db", "estoque.db", "VENDAS.DB"]);

    let gravado = json_lido(&arquivo);
    let recentes = itens(gravado.campo("recentes"));
    assert_eq!(recentes.len(), 4);
    let (virou, novo, estoque, vendas) = (&recentes[0], &recentes[1], &recentes[2], &recentes[3]);
    // o "tipo" que não é banco fica para trás; o resto vai junto
    assert_eq!(chaves(virou), ["caminho", "aberto_em", "abertas"]);
    assert_eq!(virou.campo("abertas"), Some(&Json::Lista(vec![texto_json("x")])));
    assert_eq!(chaves(novo), ["caminho", "aberto_em"]);
    assert_eq!(chaves(estoque), ["caminho", "aberto_em", "tipo", "fixado"]);
    assert_eq!(estoque.campo("tipo"), Some(&texto_json("banco")));
    assert_eq!(estoque.campo("fixado"), Some(&Json::Bool(false)));
    // a grafia nova e a hora de agora, com as chaves desconhecidas de antes
    assert_eq!(chaves(vendas), ["caminho", "aberto_em", "fixado", "cor"]);
    assert_eq!(vendas.campo("caminho"), Some(&texto_json(&caminho_exibido("c:/dados/VENDAS.DB"))));
    assert!(segundos_de_agora(vendas.campo("aberto_em").and_then(Json::como_texto).unwrap()) < 60);
    assert_eq!(vendas.campo("cor"), Some(&texto_json("azul")));
}

// Regra do Windows (caminhos `C:\…`, a mesma chave com outra caixa ou barra, o JSON gravado com `\\`):
// fora dele, os caminhos seguem o posixpath (`caminhos::testes::fora_do_windows_e_o_posixpath`).
#[test]
#[cfg(windows)]
fn recentes_mostram_os_tres_tipos_e_escondem_os_desconhecidos() {
    let tmp = PastaTemp::nova("tipos");
    let arquivo = tmp.arquivo();
    let vendas = banco(tmp.caminho(), "vendas.db", b"x");
    let dados_csv = banco(tmp.caminho(), "dados.csv", b"a;b");
    let caminho_vendas = serde_json::to_string(&texto(&vendas)).unwrap();
    let caminho_csv = serde_json::to_string(&texto(&dados_csv)).unwrap();
    gravar_texto(
        &arquivo,
        &format!(
            r#"{{"recentes": [
                {{"caminho": "C:\\Projetos", "tipo": "pasta", "aberto_em": "2026-10-03T08:00:00"}},
                {{"caminho": {caminho_vendas}, "tipo": "banco"}},
                {{"caminho": "C:\\Dados\\planilha.xlsx", "tipo": 7}},
                {{"caminho": "C:\\Dados\\nulo.db", "tipo": null}},
                {{"caminho": "C:\\Dados\\estoque.db"}},
                {{"caminho": {caminho_csv}, "tipo": "arquivo"}},
                {{"caminho": "C:\\Dados\\x.txt", "tipo": "Pasta"}}
            ]}}"#
        ),
    );
    let p = abrir(&arquivo);
    // a falta do "tipo" e os textos "banco", "pasta" e "arquivo" (só assim, minúsculos) aparecem; o resto fica
    // escondido (texto que não conhecemos, ou que não é texto)
    let itens_vistos = p.recentes();
    assert_eq!(nomes(&itens_vistos), ["Projetos", "vendas.db", "estoque.db", "dados.csv"]);
    let tipos: Vec<TipoRecente> = itens_vistos.iter().map(|i| i.tipo).collect();
    assert_eq!(tipos, [TipoRecente::Pasta, TipoRecente::Banco, TipoRecente::Banco, TipoRecente::Arquivo]);
    let existem: Vec<bool> = itens_vistos.iter().map(|i| i.existe).collect();
    assert_eq!(existem, [false, true, false, true], "a pasta não está no disco; o banco e o arquivo, sim");
    assert_eq!(itens_vistos[0].aberto_em.as_deref(), Some("2026-10-03T08:00:00"));
    assert_eq!(p.resumo().recentes, itens_vistos);

    // os escondidos continuam no arquivo, na mesma ordem
    let gravados = |arquivo: &Path| -> Vec<String> {
        itens(json_lido(arquivo).campo("recentes"))
            .iter()
            .map(|r| r.campo("caminho").and_then(Json::como_texto).unwrap().to_string())
            .collect()
    };
    p.adicionar_recente(r"C:\Dados\novo.db");
    assert_eq!(nomes(&p.recentes()), ["novo.db", "Projetos", "vendas.db", "estoque.db", "dados.csv"]);
    let (vendas, csv) = (texto(&vendas), texto(&dados_csv));
    let (planilha, nulo, x) = (r"C:\Dados\planilha.xlsx", r"C:\Dados\nulo.db", r"C:\Dados\x.txt");
    assert_eq!(
        gravados(&arquivo),
        [r"C:\Dados\novo.db", r"C:\Projetos", vendas.as_str(), planilha, nulo, r"C:\Dados\estoque.db", csv.as_str(), x]
    );
    let gravado = json_lido(&arquivo);
    let projetos = &itens(gravado.campo("recentes"))[1];
    assert_eq!(chaves(projetos), ["caminho", "aberto_em", "tipo"]);
    assert_eq!(projetos.campo("aberto_em"), Some(&texto_json("2026-10-03T08:00:00")));

    // e contam nos 15: com mais 10 bancos, saem os mais antigos, mesmo sendo bancos, pastas ou arquivos
    for i in 0..10 {
        p.adicionar_recente(format!(r"C:\Dados\b{i}.db"));
    }
    let lista = gravados(&arquivo);
    assert_eq!(lista.len(), MAX_RECENTES);
    assert!(lista.contains(&r"C:\Projetos".to_string()) && lista.contains(&planilha.to_string()));
    assert!(!lista.contains(&r"C:\Dados\estoque.db".to_string()) && !lista.contains(&csv));
    assert_eq!(p.recentes().len(), 13, "10 bancos novos, o novo, a pasta e o vendas.db; os 2 escondidos ficam de fora");

    // remover tira um escondido também (a mesma chave de caminho)
    p.remover_recente(r"c:\dados\PLANILHA.xlsx");
    assert!(!gravados(&arquivo).contains(&planilha.to_string()));
    p.remover_recente(r"c:\projetos");
    assert!(!gravados(&arquivo).contains(&r"C:\Projetos".to_string()));
}

// Regra do Windows (caminhos `C:\…`, a mesma chave com outra caixa ou barra, o JSON gravado com `\\`):
// fora dele, os caminhos seguem o posixpath (`caminhos::testes::fora_do_windows_e_o_posixpath`).
#[test]
#[cfg(windows)]
fn recente_de_pasta_e_de_arquivo_gravam_o_tipo() {
    let tmp = PastaTemp::nova("grava-tipo");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    p.adicionar_recente_de(r"C:\Projetos", TipoRecente::Pasta);
    p.adicionar_recente_de(r"C:\Dados\vendas.csv", TipoRecente::Arquivo);
    p.adicionar_recente(r"C:\Dados\vendas.db");

    // o "tipo" vem logo depois do "aberto_em"; o banco não leva a chave
    let gravado = json_lido(&arquivo);
    let recentes = itens(gravado.campo("recentes"));
    assert_eq!(recentes.len(), 3);
    assert_eq!(chaves(&recentes[0]), ["caminho", "aberto_em"]);
    assert_eq!(chaves(&recentes[1]), ["caminho", "aberto_em", "tipo"]);
    assert_eq!(recentes[1].campo("tipo"), Some(&texto_json("arquivo")));
    assert_eq!(chaves(&recentes[2]), ["caminho", "aberto_em", "tipo"]);
    assert_eq!(recentes[2].campo("tipo"), Some(&texto_json("pasta")));
    assert_eq!(recentes[2].campo("caminho"), Some(&texto_json(r"C:\Projetos")));
    let bruto = fs::read_to_string(&arquivo).unwrap();
    assert!(bruto.contains("\"tipo\": \"pasta\"") && bruto.contains("\"tipo\": \"arquivo\""));
    assert_eq!(bruto.matches("\"tipo\"").count(), 2);

    // outra instância lê os três tipos; o texto regravado é o do próprio leitor, byte a byte
    let tipos: Vec<(String, TipoRecente)> = abrir(&arquivo).recentes().into_iter().map(|r| (r.nome, r.tipo)).collect();
    assert_eq!(
        tipos,
        [
            ("vendas.db".to_string(), TipoRecente::Banco),
            ("vendas.csv".to_string(), TipoRecente::Arquivo),
            ("Projetos".to_string(), TipoRecente::Pasta)
        ]
    );
    let dados = sanear(&gravado, MAX_BANCOS_NO_HISTORICO).unwrap();
    assert_eq!(dados.serializar(), bruto);

    // abrir de novo sobe o item sem duplicar nem repetir o "tipo", com outra grafia do caminho
    p.adicionar_recente_de(r"c:/projetos", TipoRecente::Pasta);
    let recentes_de_novo = json_lido(&arquivo);
    let recentes = itens(recentes_de_novo.campo("recentes"));
    assert_eq!(recentes.len(), 3);
    assert_eq!(chaves(&recentes[0]), ["caminho", "aberto_em", "tipo"]);
    assert_eq!(nomes(&p.recentes()), ["projetos", "vendas.db", "vendas.csv"]);

    // o mesmo caminho aberto como outra coisa troca o "tipo" (e leva o resto do que o item já tinha)
    gravar_texto(
        &arquivo,
        r#"{"recentes": [{"caminho": "C:\\X\\dados", "aberto_em": null, "tipo": "pasta", "fixado": true}]}"#,
    );
    abrir(&arquivo).adicionar_recente_de(r"C:\X\dados", TipoRecente::Arquivo);
    let gravado = json_lido(&arquivo);
    let item = &itens(gravado.campo("recentes"))[0];
    assert_eq!(chaves(item), ["caminho", "aberto_em", "tipo", "fixado"]);
    assert_eq!(item.campo("tipo"), Some(&texto_json("arquivo")));
    assert_eq!(item.campo("fixado"), Some(&Json::Bool(true)));
    // o mesmo tipo fica onde estava: o "tipo" escrito depois de outra chave não muda de lugar
    gravar_texto(
        &arquivo,
        r#"{"recentes": [{"caminho": "C:\\X\\dados", "aberto_em": null, "fixado": true, "tipo": "pasta"}]}"#,
    );
    abrir(&arquivo).adicionar_recente_de(r"C:\X\dados", TipoRecente::Pasta);
    let gravado = json_lido(&arquivo);
    assert_eq!(chaves(&itens(gravado.campo("recentes"))[0]), ["caminho", "aberto_em", "fixado", "tipo"]);
    // e um "tipo" que não conhecemos é contradição para qualquer um dos três
    gravar_texto(&arquivo, r#"{"recentes": [{"caminho": "C:\\X\\dados", "tipo": "atalho"}]}"#);
    abrir(&arquivo).adicionar_recente_de(r"C:\X\dados", TipoRecente::Pasta);
    let gravado = json_lido(&arquivo);
    let item = &itens(gravado.campo("recentes"))[0];
    assert_eq!(chaves(item), ["caminho", "aberto_em", "tipo"]);
    assert_eq!(item.campo("tipo"), Some(&texto_json("pasta")));
}

#[test]
fn recente_de_banco_nao_grava_o_tipo() {
    let tmp = PastaTemp::nova("banco-sem-tipo");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    p.adicionar_recente(r"C:\Dados\vendas.db");
    p.adicionar_recente_de(r"C:\Dados\estoque.db", TipoRecente::Banco);
    p.registrar_sql(r"C:\Dados\vendas.db", "SELECT 1", None, None);
    p.atualizar(Mudanca::tema(Tema::Escuro)).unwrap();
    // quem só abre bancos tem o arquivo sem chave nova nenhuma, nem o "tipo"
    let bruto = fs::read_to_string(&arquivo).unwrap();
    assert!(!bruto.contains("tipo"));
    let gravado = json_lido(&arquivo);
    assert_eq!(chaves(&gravado), ["versao", "tema", "exportacao", "ultimo_diretorio", "recentes", "historico_sql"]);
    for item in itens(gravado.campo("recentes")) {
        assert_eq!(chaves(item), ["caminho", "aberto_em"]);
    }
    let dados = sanear(&gravado, MAX_BANCOS_NO_HISTORICO).unwrap();
    assert_eq!(sem_extras(&dados).serializar(), bruto);
    assert!(p.recentes().iter().all(|r| r.tipo == TipoRecente::Banco));

    // um "tipo": "banco" que já estava fica; qualquer outro "tipo" sai quando o item é aberto como banco
    gravar_texto(
        &arquivo,
        r#"{"recentes": [
            {"caminho": "C:\\A\\a.db", "aberto_em": null, "tipo": "banco", "fixado": true},
            {"caminho": "C:\\A\\b.db", "aberto_em": null, "tipo": "pasta"},
            {"caminho": "C:\\A\\c.db", "aberto_em": null, "tipo": "arquivo"},
            {"caminho": "C:\\A\\d.db", "aberto_em": null, "tipo": "atalho"}
        ]}"#,
    );
    let p = abrir(&arquivo);
    for nome in ["a", "b", "c", "d"] {
        p.adicionar_recente(format!(r"C:\A\{nome}.db"));
    }
    let gravado = json_lido(&arquivo);
    let recentes = itens(gravado.campo("recentes"));
    // a ordem agora é d, c, b, a
    assert_eq!(chaves(&recentes[3]), ["caminho", "aberto_em", "tipo", "fixado"]);
    assert_eq!(recentes[3].campo("tipo"), Some(&texto_json("banco")));
    for item in &recentes[..3] {
        assert_eq!(chaves(item), ["caminho", "aberto_em"]);
    }
}

#[test]
fn limpar_recentes_tira_os_tres_tipos() {
    let tmp = PastaTemp::nova("limpar-tipos");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    p.adicionar_recente(r"C:\Dados\vendas.db");
    p.adicionar_recente_de(r"C:\Projetos", TipoRecente::Pasta);
    p.adicionar_recente_de(r"C:\Dados\vendas.csv", TipoRecente::Arquivo);
    p.limpar_recentes();
    assert!(p.recentes().is_empty());
    assert_eq!(json_do_arquivo(&arquivo)["recentes"], serde_json::json!([]));

    // o de tipo desconhecido (de um DataBox mais novo) fica, e sem nenhum conhecido nada é gravado de novo
    let original = r#"{"recentes": [{"caminho": "C:\\Dados\\a.xlsx", "tipo": "planilha"}, {"caminho": "C:\\Dados\\n.db", "tipo": 3}]}"#;
    gravar_texto(&arquivo, original);
    let p = abrir(&arquivo);
    p.limpar_recentes();
    assert_eq!(fs::read_to_string(&arquivo).unwrap(), original, "nada a limpar, nada a gravar");
    p.adicionar_recente_de(r"C:\Projetos", TipoRecente::Pasta);
    p.adicionar_recente(r"C:\Dados\vendas.db");
    p.limpar_recentes();
    let gravado = json_lido(&arquivo);
    let caminhos: Vec<&str> =
        itens(gravado.campo("recentes")).iter().filter_map(|r| r.campo("caminho").and_then(Json::como_texto)).collect();
    assert_eq!(caminhos, [r"C:\Dados\a.xlsx", r"C:\Dados\n.db"]);
}

#[test]
fn tipo_recente_como_str_e_de_texto() {
    let pares = [(TipoRecente::Banco, "banco"), (TipoRecente::Pasta, "pasta"), (TipoRecente::Arquivo, "arquivo")];
    assert_eq!(pares.map(|(t, _)| t), TipoRecente::TODOS);
    for (tipo, texto) in pares {
        assert_eq!(tipo.como_str(), texto);
        assert_eq!(TipoRecente::de_texto(texto).unwrap(), tipo);
        assert_eq!(texto.parse::<TipoRecente>().unwrap(), tipo);
        assert_eq!(tipo.to_string(), texto);
    }
    assert_eq!(TipoRecente::default(), TipoRecente::Banco);
    let erro = TipoRecente::de_texto("Pasta").unwrap_err();
    assert_eq!(erro.status, 400);
    assert_eq!(erro.mensagem, "Tipo de recente desconhecido: 'Pasta'. Use banco, pasta ou arquivo.");
}

// ---- a pasta aberta ---------------------------------------------------------------------------------

#[test]
fn pasta_aberta_grava_e_volta() {
    let tmp = PastaTemp::nova("pasta-aberta");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    assert_eq!(p.pasta_aberta(), None);
    // sem pasta aberta, nenhuma das outras mudanças cria a chave
    p.atualizar(Mudanca::tema(Tema::Escuro)).unwrap();
    p.adicionar_recente_de(r"C:\Projetos", TipoRecente::Pasta);
    assert!(!fs::read_to_string(&arquivo).unwrap().contains("pasta_aberta"));

    let pasta = tmp.caminho().join("Projetos"); // a pasta nem precisa existir: o disco é de quem a abre
    p.atualizar(Mudanca::pasta_aberta(Some(pasta.clone()))).unwrap();
    assert_eq!(p.pasta_aberta().as_deref(), Some(caminho_exibido(&pasta).as_str()));
    let gravado = json_lido(&arquivo);
    assert_eq!(
        chaves(&gravado),
        ["versao", "tema", "exportacao", "ultimo_diretorio", "recentes", "historico_sql", "pasta_aberta"]
    );
    assert_eq!(gravado.campo("pasta_aberta"), Some(&texto_json(&texto(&pasta))));
    assert_eq!(p.tema(), Tema::Escuro, "o resto fica como estava");
    assert_eq!(abrir(&arquivo).pasta_aberta().as_deref(), Some(texto(&pasta).as_str()), "outra instância lê o mesmo");

    // as outras mudanças a levam junto; a aparência e as larguras entram antes dela
    p.atualizar(Mudanca::aparencia(MudancaAparencia { tamanho_codigo: Some(13.0), ..Default::default() })).unwrap();
    p.atualizar(Mudanca::paineis(MudancaLarguraPaineis { explorador: Some(300.0), propriedades: None })).unwrap();
    p.registrar_sql(r"C:\Dados\vendas.db", "SELECT 1", None, None);
    p.adicionar_recente(r"C:\Dados\vendas.db");
    let gravado = json_lido(&arquivo);
    assert_eq!(
        chaves(&gravado),
        [
            "versao",
            "tema",
            "exportacao",
            "ultimo_diretorio",
            "recentes",
            "historico_sql",
            "aparencia",
            "larguras_paineis",
            "pasta_aberta"
        ]
    );
    assert_eq!(abrir(&arquivo).pasta_aberta().as_deref(), Some(texto(&pasta).as_str()));
    let dados = sanear(&gravado, MAX_BANCOS_NO_HISTORICO).unwrap();
    assert_eq!(dados.serializar(), fs::read_to_string(&arquivo).unwrap());

    // abrir outra pasta troca a chave; um caminho relativo vira absoluto, como no ultimo_diretorio
    let outra = format!("rel_{}", tmp.caminho().file_name().unwrap().to_string_lossy());
    p.atualizar(Mudanca::pasta_aberta(Some(PathBuf::from(&outra)))).unwrap();
    assert_eq!(p.pasta_aberta(), Some(texto(&std::env::current_dir().unwrap().join(&outra))));
    assert_eq!(chaves(&json_lido(&arquivo)).iter().filter(|c| **c == "pasta_aberta").count(), 1);

    // junto com outra mudança, na mesma gravação
    p.atualizar(Mudanca { tema: Some(Tema::Claro), pasta_aberta: Some(Some(pasta.clone())), ..Mudanca::default() })
        .unwrap();
    assert_eq!((p.tema(), p.pasta_aberta()), (Tema::Claro, Some(texto(&pasta))));
}

// Regra do Windows (caminhos `C:\…`, a mesma chave com outra caixa ou barra, o JSON gravado com `\\`):
// fora dele, os caminhos seguem o posixpath (`caminhos::testes::fora_do_windows_e_o_posixpath`).
#[test]
#[cfg(windows)]
fn fechar_pasta_apaga_a_chave_e_o_arquivo_volta_ao_de_antes() {
    let tmp = PastaTemp::nova("fechar-pasta");
    let arquivo = tmp.arquivo();
    let original = GRAVADO_SEM_EXTRAS.replace("<DEL>", "\u{7f}").replace("<LS>", "\u{2028}");
    gravar_texto(&arquivo, &original);
    let p = abrir(&arquivo);
    p.atualizar(Mudanca::pasta_aberta(Some(PathBuf::from(r"C:\Projetos")))).unwrap();
    let com_pasta = fs::read_to_string(&arquivo).unwrap();
    assert!(com_pasta.ends_with("  },\n  \"pasta_aberta\": \"C:\\\\Projetos\"\n}"), "{com_pasta}");
    assert_eq!(p.pasta_aberta().as_deref(), Some(r"C:\Projetos"));

    // o Fechar pasta: a chave some, e o texto volta letra por letra ao de antes
    p.atualizar(Mudanca::pasta_aberta(None)).unwrap();
    assert_eq!(p.pasta_aberta(), None);
    assert_eq!(fs::read_to_string(&arquivo).unwrap(), original);
    assert_eq!(abrir(&arquivo).pasta_aberta(), None);
    // fechar sem pasta aberta também deixa o arquivo como estava
    p.atualizar(Mudanca::pasta_aberta(None)).unwrap();
    assert_eq!(fs::read_to_string(&arquivo).unwrap(), original);

    // os recentes ficam: o Fechar pasta não mexe neles
    p.adicionar_recente_de(r"C:\Projetos", TipoRecente::Pasta);
    p.atualizar(Mudanca::pasta_aberta(Some(PathBuf::from(r"C:\Projetos")))).unwrap();
    p.atualizar(Mudanca::pasta_aberta(None)).unwrap();
    assert_eq!(p.recentes()[0].tipo, TipoRecente::Pasta);
}

#[test]
fn pasta_aberta_estranha_e_ignorada() {
    // valor que não é texto, ou é vazio: nenhuma pasta aberta, e a chave some na próxima gravação (sem duplicar)
    for valor in ["7", "null", "[]", r#"["C:\\x"]"#, "{}", "true", r#""""#, r#""   ""#] {
        let tmp = PastaTemp::nova("pasta-aberta-estranha");
        let arquivo = tmp.arquivo();
        gravar_texto(&arquivo, &format!(r#"{{"tema": "dark", "pasta_aberta": {valor}}}"#));
        let p = abrir(&arquivo);
        assert_eq!(p.pasta_aberta(), None, "{valor}");
        assert_eq!(p.tema(), Tema::Escuro, "{valor}");
        p.atualizar(Mudanca::default()).unwrap();
        assert!(json_lido(&arquivo).campo("pasta_aberta").is_none(), "{valor}");
    }
    // o texto vale como veio (não vira abspath na leitura), e é chave conhecida, não um extra
    let tmp = PastaTemp::nova("pasta-aberta-conhecida");
    let arquivo = tmp.arquivo();
    gravar_texto(&arquivo, r#"{"tema": "dark", "pasta_aberta": "C:\\Projetos", "outra": 1}"#);
    let p = abrir(&arquivo);
    assert_eq!(p.pasta_aberta().as_deref(), Some(r"C:\Projetos"));
    p.atualizar(Mudanca::tema(Tema::Claro)).unwrap();
    let gravado = json_lido(&arquivo);
    assert_eq!(chaves(&gravado).iter().filter(|c| **c == "pasta_aberta").count(), 1);
    assert_eq!(chaves(&gravado).last(), Some(&"outra"));
    assert!(sanear(&gravado, MAX_BANCOS_NO_HISTORICO).unwrap().extras.iter().all(|(k, _)| k != "pasta_aberta"));
}

// Regra do Windows (caminhos `C:\…`, a mesma chave com outra caixa ou barra, o JSON gravado com `\\`):
// fora dele, os caminhos seguem o posixpath (`caminhos::testes::fora_do_windows_e_o_posixpath`).
#[test]
#[cfg(windows)]
fn pasta_aberta_vazia_recusa() {
    let tmp = PastaTemp::nova("pasta-aberta-vazia");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    for vazio in ["", "   ", "\t"] {
        let e = p.atualizar(Mudanca::pasta_aberta(Some(PathBuf::from(vazio)))).unwrap_err();
        assert_eq!(e.status, 400);
        assert_eq!(e.mensagem, "A pasta aberta precisa ser um caminho.");
    }
    assert_eq!(p.pasta_aberta(), None);
    assert!(!arquivo.exists(), "nada foi gravado");
    // recusada junto com outra mudança, nenhuma das duas vale
    let e = p
        .atualizar(Mudanca { tema: Some(Tema::Escuro), pasta_aberta: Some(Some(PathBuf::new())), ..Mudanca::default() })
        .unwrap_err();
    assert_eq!(e.mensagem, "A pasta aberta precisa ser um caminho.");
    assert_eq!(p.tema(), Tema::Sistema);
    // e a pasta que estava aberta continua
    p.atualizar(Mudanca::pasta_aberta(Some(PathBuf::from(r"C:\Projetos")))).unwrap();
    p.atualizar(Mudanca::pasta_aberta(Some(PathBuf::from(" ")))).unwrap_err();
    assert_eq!(p.pasta_aberta().as_deref(), Some(r"C:\Projetos"));
}

#[test]
fn sem_extras_o_texto_e_o_mesmo() {
    // o arquivo sem chave desconhecida: lido e regravado, sai letra por letra igual
    let original = GRAVADO_SEM_EXTRAS.replace("<DEL>", "\u{7f}").replace("<LS>", "\u{2028}");
    let dados = sanear(&json::ler(original.as_bytes()).unwrap(), MAX_BANCOS_NO_HISTORICO).unwrap();
    assert!(dados.extras.is_empty() && dados.recentes.iter().all(|r| r.extras.is_empty()));
    assert_eq!(dados.serializar(), original);

    // também pelo caminho de verdade: uma mudança que não muda nada regrava o mesmo texto
    let tmp = PastaTemp::nova("sem-extras");
    let arquivo = tmp.arquivo();
    gravar_texto(&arquivo, &original);
    abrir(&arquivo).atualizar(Mudanca::default()).unwrap();
    assert_eq!(fs::read_to_string(&arquivo).unwrap(), original);

    // e com extras, tirá-los dá o texto sem extras
    let novo = sanear(&json::ler(GRAVADO_POR_UMA_VERSAO_NOVA.as_bytes()).unwrap(), MAX_BANCOS_NO_HISTORICO).unwrap();
    let mut sem = sem_extras(&novo);
    sem.tema = Tema::Claro;
    let esperado = "{\n  \"versao\": 1,\n  \"tema\": \"light\",\n  \"exportacao\": {\n    \"csv_separador\": \";\",\n    \
                    \"csv_decimal\": \",\",\n    \"json_indentado\": true\n  },\n  \"ultimo_diretorio\": null,\n  \
                    \"recentes\": [\n    {\n      \"caminho\": \"C:\\\\Dados\\\\vendas.db\",\n      \"aberto_em\": \
                    \"2026-10-07T09:08:07\"\n    },\n    {\n      \"caminho\": \"C:\\\\Projetos\",\n      \"aberto_em\": \
                    null\n    }\n  ],\n  \"historico_sql\": {}\n}";
    assert_eq!(sem.serializar(), esperado);
}

// ---- aparência do código ----------------------------------------------------------------------------

#[test]
fn aparencia_padrao_nao_vai_para_o_arquivo() {
    let tmp = PastaTemp::nova("aparencia-padrao");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    assert_eq!(p.aparencia(), Aparencia::default());
    assert_eq!(
        Aparencia::default(),
        Aparencia { cores_codigo: CoresCodigo::DataBox, fonte_codigo: None, tamanho_codigo: 12.5 }
    );
    // as outras mudanças não criam a chave
    p.atualizar(Mudanca::tema(Tema::Escuro)).unwrap();
    p.adicionar_recente(r"C:\Dados\vendas.db");
    p.registrar_sql(r"C:\Dados\vendas.db", "SELECT 1", None, None);
    let gravado = json_lido(&arquivo);
    assert_eq!(chaves(&gravado), ["versao", "tema", "exportacao", "ultimo_diretorio", "recentes", "historico_sql"]);
    assert_eq!(p.resumo().aparencia, Aparencia::default());
    let dados = sanear(&gravado, MAX_BANCOS_NO_HISTORICO).unwrap();
    assert_eq!(dados.aparencia, None);
    assert_eq!(dados.serializar(), fs::read_to_string(&arquivo).unwrap());
    // e o padrão de fábrica continua o texto de sempre
    assert!(!Dados::padrao().serializar().contains("aparencia"));
}

#[test]
fn aparencia_grava_e_volta() {
    let tmp = PastaTemp::nova("aparencia-grava");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    p.atualizar(Mudanca::aparencia(MudancaAparencia {
        cores_codigo: Some(CoresCodigo::Monokai),
        ..Default::default()
    }))
    .unwrap();
    // só as cores mudaram; o resto fica no padrão, mas a chave inteira entra, depois do histórico
    let esperado_monokai = Aparencia { cores_codigo: CoresCodigo::Monokai, ..Aparencia::default() };
    assert_eq!(p.aparencia(), esperado_monokai);
    let texto_arquivo = fs::read_to_string(&arquivo).unwrap();
    assert!(
        texto_arquivo.ends_with(
            "\"historico_sql\": {},\n  \"aparencia\": {\n    \"cores_codigo\": \"monokai\",\n    \"fonte_codigo\": null,\n    \
             \"tamanho_codigo\": 12.5\n  }\n}"
        ),
        "{texto_arquivo}"
    );

    // mesclada: família e tamanho, sem mexer nas cores
    p.atualizar(Mudanca::aparencia(MudancaAparencia {
        fonte_codigo: Some(Some("  Cascadia Mono ".to_string())),
        tamanho_codigo: Some(12.3),
        ..Default::default()
    }))
    .unwrap();
    let outra = abrir(&arquivo);
    let esperada = Aparencia {
        cores_codigo: CoresCodigo::Monokai,
        fonte_codigo: Some("Cascadia Mono".to_string()),
        tamanho_codigo: 12.3,
    };
    assert_eq!(outra.aparencia(), esperada);
    assert_eq!(outra.resumo().aparencia, esperada);
    // o f32 sai em centésimos, sem o ruído da conversão
    assert_eq!(json_do_arquivo(&arquivo)["aparencia"]["tamanho_codigo"], serde_json::json!(12.3));
    assert_eq!(json_do_arquivo(&arquivo)["aparencia"]["fonte_codigo"], "Cascadia Mono");

    // de volta ao padrão: a chave fica (a pessoa escolheu), com os valores de fábrica
    outra.atualizar(Mudanca::aparencia(Aparencia::default().into())).unwrap();
    assert_eq!(p.aparencia(), Aparencia::default());
    let gravado = json_do_arquivo(&arquivo);
    assert_eq!(
        gravado["aparencia"],
        serde_json::json!({"cores_codigo": "databox", "fonte_codigo": null, "tamanho_codigo": 12.5})
    );
    // as outras chaves não mudaram por causa dela
    assert_eq!(gravado["tema"], "system");

    // tamanho fora da faixa é recusado sem gravar nada
    for ruim in [7.9, 24.5, f32::NAN, f32::INFINITY] {
        let erro = p
            .atualizar(Mudanca::aparencia(MudancaAparencia { tamanho_codigo: Some(ruim), ..Default::default() }))
            .unwrap_err();
        assert_eq!(erro.status, 400);
        assert_eq!(erro.mensagem, "O tamanho da fonte do código precisa ficar entre 8 e 24 px.");
    }
    assert_eq!(p.aparencia(), Aparencia::default());
    for limite in [8.0, 24.0] {
        p.atualizar(Mudanca::aparencia(MudancaAparencia { tamanho_codigo: Some(limite), ..Default::default() }))
            .unwrap();
        assert_eq!(p.aparencia().tamanho_codigo, limite);
    }
}

#[test]
fn aparencia_estranha_volta_ao_padrao() {
    let casos = [
        // (o que está no arquivo, o que vale)
        (r#"{"cores_codigo": "roxo", "fonte_codigo": "Lucida Console", "tamanho_codigo": 14}"#, {
            Aparencia { fonte_codigo: Some("Lucida Console".to_string()), tamanho_codigo: 14.0, ..Default::default() }
        }),
        (r#"{"cores_codigo": "solarized_escuro", "fonte_codigo": 7, "tamanho_codigo": "grande"}"#, {
            Aparencia { cores_codigo: CoresCodigo::SolarizedEscuro, ..Default::default() }
        }),
        (r#"{"cores_codigo": "MONOKAI", "fonte_codigo": "   ", "tamanho_codigo": NaN}"#, Aparencia::default()),
        (r#"{}"#, Aparencia::default()),
        (r#"{"cores_codigo": "alto_contraste"}"#, {
            Aparencia { cores_codigo: CoresCodigo::AltoContraste, ..Default::default() }
        }),
    ];
    for (aparencia, esperada) in casos {
        let tmp = PastaTemp::nova("aparencia-estranha");
        let arquivo = tmp.arquivo();
        gravar_texto(&arquivo, &format!(r#"{{"tema": "dark", "aparencia": {aparencia}}}"#));
        let p = abrir(&arquivo);
        assert_eq!(p.aparencia(), esperada, "{aparencia}");
        assert_eq!(p.tema(), Tema::Escuro);
        assert!(!arquivo.with_file_name("preferencias.json.corrompido").exists());
    }

    // "aparencia" que não é objeto conta como não escolhida, e some na próxima gravação
    for valor in ["\"monokai\"", "null", "[1, 2]", "12.5"] {
        let tmp = PastaTemp::nova("aparencia-nao-objeto");
        let arquivo = tmp.arquivo();
        gravar_texto(&arquivo, &format!(r#"{{"tema": "dark", "aparencia": {valor}}}"#));
        let p = abrir(&arquivo);
        assert_eq!(p.aparencia(), Aparencia::default());
        p.atualizar(Mudanca::default()).unwrap();
        assert!(json_lido(&arquivo).campo("aparencia").is_none(), "{valor}");
    }

    // as chaves da aparência que esta versão não conhece ficam, depois das conhecidas
    let tmp = PastaTemp::nova("aparencia-extras");
    let arquivo = tmp.arquivo();
    gravar_texto(&arquivo, r#"{"aparencia": {"ligaduras": true, "cores_codigo": "monokai"}, "outra": 1}"#);
    let p = abrir(&arquivo);
    p.atualizar(Mudanca::aparencia(MudancaAparencia { tamanho_codigo: Some(13.0), ..Default::default() })).unwrap();
    let gravado = json_lido(&arquivo);
    let aparencia = gravado.campo("aparencia").unwrap();
    assert_eq!(chaves(aparencia), ["cores_codigo", "fonte_codigo", "tamanho_codigo", "ligaduras"]);
    assert_eq!(aparencia.campo("ligaduras"), Some(&Json::Bool(true)));
    assert_eq!(aparencia.campo("tamanho_codigo"), Some(&Json::Real(13.0)));
    assert_eq!(chaves(&gravado).last(), Some(&"outra"));
}

#[test]
fn tamanho_do_codigo_fora_da_faixa_volta_ao_padrao() {
    for (tamanho, vale) in [
        ("7.99", 12.5),
        ("8", 8.0),
        ("8.0", 8.0),
        ("16", 16.0),
        ("24", 24.0),
        ("24.01", 12.5),
        ("-12", 12.5),
        ("0", 12.5),
        ("1e400", 12.5),
        ("Infinity", 12.5),
        ("true", 12.5),
        ("null", 12.5),
    ] {
        let tmp = PastaTemp::nova("aparencia-tamanho");
        let arquivo = tmp.arquivo();
        gravar_texto(&arquivo, &format!(r#"{{"aparencia": {{"tamanho_codigo": {tamanho}}}}}"#));
        assert_eq!(abrir(&arquivo).aparencia().tamanho_codigo, vale, "{tamanho}");
    }
}

// ---- larguras dos painéis ---------------------------------------------------------------------------

#[test]
fn paineis_padrao_nao_vao_para_o_arquivo() {
    let tmp = PastaTemp::nova("paineis-padrao");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    assert_eq!(p.paineis(), LarguraPaineis::default());
    assert_eq!(p.paineis(), LarguraPaineis { explorador: None, propriedades: None });
    // as outras mudanças não criam a chave
    p.atualizar(Mudanca::tema(Tema::Escuro)).unwrap();
    p.adicionar_recente(r"C:\Dados\vendas.db");
    p.registrar_sql(r"C:\Dados\vendas.db", "SELECT 1", None, None);
    let gravado = json_lido(&arquivo);
    assert_eq!(chaves(&gravado), ["versao", "tema", "exportacao", "ultimo_diretorio", "recentes", "historico_sql"]);
    let dados = sanear(&gravado, MAX_BANCOS_NO_HISTORICO).unwrap();
    assert_eq!(dados.paineis, None);
    assert_eq!(dados.serializar(), fs::read_to_string(&arquivo).unwrap());
    assert!(!Dados::padrao().serializar().contains("larguras_paineis"));
    // sem a chave no arquivo, as larguras são as padrão (a casca decide quais são)
    gravar_texto(&arquivo, r#"{"tema": "dark", "aparencia": {"tamanho_codigo": 14}}"#);
    assert_eq!(abrir(&arquivo).paineis(), LarguraPaineis::default());
}

#[test]
fn paineis_gravam_e_voltam_em_pontos_inteiros() {
    let tmp = PastaTemp::nova("paineis-grava");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    p.atualizar(Mudanca::paineis(MudancaLarguraPaineis { explorador: Some(300.4), propriedades: None })).unwrap();
    assert_eq!(p.paineis(), LarguraPaineis { explorador: Some(300.0), propriedades: None });
    // só a que mudou entra no arquivo, depois do historico_sql
    let gravado = json_lido(&arquivo);
    assert_eq!(
        chaves(&gravado),
        ["versao", "tema", "exportacao", "ultimo_diretorio", "recentes", "historico_sql", "larguras_paineis"]
    );
    assert_eq!(chaves(gravado.campo("larguras_paineis").unwrap()), ["explorador"]);
    assert_eq!(json_do_arquivo(&arquivo)["larguras_paineis"], serde_json::json!({"explorador": 300}));
    // a outra se mescla; a primeira continua
    p.atualizar(Mudanca::paineis(MudancaLarguraPaineis { explorador: None, propriedades: Some(240.5) })).unwrap();
    let esperadas = LarguraPaineis { explorador: Some(300.0), propriedades: Some(241.0) };
    assert_eq!(p.paineis(), esperadas);
    assert_eq!(
        json_do_arquivo(&arquivo)["larguras_paineis"],
        serde_json::json!({"explorador": 300, "propriedades": 241})
    );
    // outra instância lê o mesmo; e a aparência entra antes dos paineis
    let outra = abrir(&arquivo);
    assert_eq!(outra.paineis(), esperadas);
    outra.atualizar(Mudanca::aparencia(MudancaAparencia { tamanho_codigo: Some(13.0), ..Default::default() })).unwrap();
    let gravado = json_lido(&arquivo);
    assert_eq!(
        chaves(&gravado),
        [
            "versao",
            "tema",
            "exportacao",
            "ultimo_diretorio",
            "recentes",
            "historico_sql",
            "aparencia",
            "larguras_paineis"
        ]
    );
    assert_eq!(abrir(&arquivo).paineis(), esperadas);
    // o texto regravado é o do próprio leitor, byte a byte
    let dados = sanear(&gravado, MAX_BANCOS_NO_HISTORICO).unwrap();
    assert_eq!(dados.serializar(), fs::read_to_string(&arquivo).unwrap());
}

#[test]
fn paineis_fora_da_faixa_sao_recusados_e_lidos_como_padrao() {
    let tmp = PastaTemp::nova("paineis-faixa");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    for ruim in [f32::NAN, f32::INFINITY, 0.0, 119.0, 2000.5, -300.0] {
        let e = p
            .atualizar(Mudanca::paineis(MudancaLarguraPaineis { explorador: Some(ruim), ..Default::default() }))
            .unwrap_err();
        assert_eq!(e.mensagem, "A largura do painel precisa ficar entre 120 e 2000 pt.", "{ruim}");
        let e = p
            .atualizar(Mudanca::paineis(MudancaLarguraPaineis { propriedades: Some(ruim), ..Default::default() }))
            .unwrap_err();
        assert_eq!(e.mensagem, "A largura do painel precisa ficar entre 120 e 2000 pt.", "{ruim}");
    }
    assert_eq!(p.paineis(), LarguraPaineis::default());
    assert!(!arquivo.exists(), "nada foi gravado");
    for limite in [120.0, 2000.0] {
        p.atualizar(Mudanca::paineis(MudancaLarguraPaineis { explorador: Some(limite), ..Default::default() }))
            .unwrap();
        assert_eq!(p.paineis().explorador, Some(limite));
    }
    // lido do arquivo, o estranho volta ao padrão sem alarde (cada largura por si)
    let so = |explorador, propriedades| LarguraPaineis { explorador, propriedades };
    for (texto, esperada) in [
        (r#"{"explorador": 301, "propriedades": 330.4}"#, so(Some(301.0), Some(330.0))),
        (r#"{"explorador": 119, "propriedades": 330}"#, so(None, Some(330.0))),
        (r#"{"explorador": "300", "propriedades": null}"#, LarguraPaineis::default()),
        (r#"{"explorador": 1e400, "propriedades": -5}"#, LarguraPaineis::default()),
        (r#"{"explorador": true, "propriedades": [300]}"#, LarguraPaineis::default()),
        (r#"{}"#, LarguraPaineis::default()),
    ] {
        let tmp = PastaTemp::nova("paineis-estranhos");
        let arquivo = tmp.arquivo();
        gravar_texto(&arquivo, &format!(r#"{{"tema": "dark", "larguras_paineis": {texto}}}"#));
        assert_eq!(abrir(&arquivo).paineis(), esperada, "{texto}");
    }
    // um "larguras_paineis" que não é objeto conta como não escolhido, e some na próxima gravação
    for valor in ["300", "[300, 272]", "null", r#""largo""#] {
        let tmp = PastaTemp::nova("paineis-nao-objeto");
        let arquivo = tmp.arquivo();
        gravar_texto(&arquivo, &format!(r#"{{"tema": "dark", "larguras_paineis": {valor}}}"#));
        let p = abrir(&arquivo);
        assert_eq!(p.paineis(), LarguraPaineis::default(), "{valor}");
        p.atualizar(Mudanca::default()).unwrap();
        assert!(json_lido(&arquivo).campo("larguras_paineis").is_none(), "{valor}");
    }
    // as chaves dos paineis que esta versão não conhece ficam, depois das conhecidas, e a chave de fora também
    let tmp = PastaTemp::nova("paineis-extras");
    let arquivo = tmp.arquivo();
    gravar_texto(&arquivo, r#"{"larguras_paineis": {"historico": 280, "propriedades": 250}, "outra": 1}"#);
    let p = abrir(&arquivo);
    p.atualizar(Mudanca::paineis(MudancaLarguraPaineis { explorador: Some(310.0), ..Default::default() })).unwrap();
    let gravado = json_lido(&arquivo);
    let paineis = gravado.campo("larguras_paineis").unwrap();
    assert_eq!(chaves(paineis), ["explorador", "propriedades", "historico"]);
    assert_eq!(paineis.campo("historico"), Some(&Json::Inteiro(280)));
    assert_eq!(chaves(&gravado).last(), Some(&"outra"));
}

#[test]
fn paineis_sao_chave_conhecida_e_nao_um_extra() {
    // um DataBox que já grava os paineis não os trata como chave de uma versão mais nova (nem os duplica)
    let tmp = PastaTemp::nova("paineis-conhecida");
    let arquivo = tmp.arquivo();
    gravar_texto(&arquivo, r#"{"tema": "dark", "larguras_paineis": {"explorador": 290}}"#);
    let p = abrir(&arquivo);
    p.atualizar(Mudanca::tema(Tema::Claro)).unwrap();
    let gravado = json_lido(&arquivo);
    assert_eq!(chaves(&gravado).iter().filter(|c| **c == "larguras_paineis").count(), 1);
    assert_eq!(p.paineis(), LarguraPaineis { explorador: Some(290.0), propriedades: None });
    let dados = sanear(&gravado, MAX_BANCOS_NO_HISTORICO).unwrap();
    assert!(dados.extras.is_empty());
}

#[test]
fn cores_codigo_como_str_e_de_texto() {
    let pares = [
        (CoresCodigo::DataBox, "databox"),
        (CoresCodigo::AltoContraste, "alto_contraste"),
        (CoresCodigo::SolarizedEscuro, "solarized_escuro"),
        (CoresCodigo::Monokai, "monokai"),
    ];
    assert_eq!(pares.map(|(c, _)| c), CoresCodigo::TODAS);
    for (cores, texto) in pares {
        assert_eq!(cores.como_str(), texto);
        assert_eq!(CoresCodigo::de_texto(texto).unwrap(), cores);
        assert_eq!(texto.parse::<CoresCodigo>().unwrap(), cores);
        assert_eq!(cores.to_string(), texto);
    }
    assert_eq!(CoresCodigo::default(), CoresCodigo::DataBox);
    let erro = CoresCodigo::de_texto("Monokai").unwrap_err();
    assert_eq!(erro.status, 400);
    assert_eq!(
        erro.mensagem,
        "Cores do código desconhecidas: 'Monokai'. Use databox, alto_contraste, solarized_escuro ou monokai."
    );
}

#[test]
fn lugares_dos_paineis_gravam_mesclam_e_voltam() {
    let tmp = PastaTemp::nova("lugares-grava");
    let arquivo = tmp.arquivo();
    let p = abrir(&arquivo);
    // antes de a pessoa mexer: tudo padrão e sem a chave no arquivo
    assert_eq!(p.lugares(), LugaresPaineis::default());
    p.atualizar(Mudanca::lugares(LugaresPaineis { propriedades: Some(LugarPainel::Janela), ..Default::default() }))
        .unwrap();
    assert_eq!(p.lugares(), LugaresPaineis { propriedades: Some(LugarPainel::Janela), ..Default::default() });
    assert_eq!(json_do_arquivo(&arquivo)["lugares_paineis"], serde_json::json!({"propriedades": "janela"}));
    // o outro se mescla; o primeiro continua; a chave vem depois das larguras
    p.atualizar(Mudanca::paineis(MudancaLarguraPaineis { explorador: Some(300.0), propriedades: None })).unwrap();
    p.atualizar(Mudanca::lugares(LugaresPaineis { historico: Some(LugarPainel::Direita), ..Default::default() }))
        .unwrap();
    let esperados = LugaresPaineis {
        explorador: None,
        historico: Some(LugarPainel::Direita),
        propriedades: Some(LugarPainel::Janela),
    };
    assert_eq!(abrir(&arquivo).lugares(), esperados);
    let gravado = json_lido(&arquivo);
    assert_eq!(
        chaves(&gravado),
        [
            "versao",
            "tema",
            "exportacao",
            "ultimo_diretorio",
            "recentes",
            "historico_sql",
            "larguras_paineis",
            "lugares_paineis"
        ]
    );
    assert_eq!(chaves(gravado.campo("lugares_paineis").unwrap()), ["historico", "propriedades"]);
    let dados = sanear(&gravado, MAX_BANCOS_NO_HISTORICO).unwrap();
    assert_eq!(dados.serializar(), fs::read_to_string(&arquivo).unwrap());
}

#[test]
fn lugares_estranhos_voltam_ao_padrao_e_as_chaves_novas_ficam() {
    let tmp = PastaTemp::nova("lugares-estranhos");
    let arquivo = tmp.arquivo();
    gravar_texto(
        &arquivo,
        r#"{"tema": "dark", "lugares_paineis": {"explorador": "teto", "historico": 3, "propriedades": "direita", "graficos": "janela"}}"#,
    );
    let p = abrir(&arquivo);
    assert_eq!(p.lugares(), LugaresPaineis { propriedades: Some(LugarPainel::Direita), ..Default::default() });
    // regravado, o desconhecido continua
    p.atualizar(Mudanca::lugares(LugaresPaineis { explorador: Some(LugarPainel::Esquerda), ..Default::default() }))
        .unwrap();
    assert_eq!(
        json_do_arquivo(&arquivo)["lugares_paineis"],
        serde_json::json!({"explorador": "esquerda", "propriedades": "direita", "graficos": "janela"})
    );
    // um "lugares_paineis" que não é objeto conta como não escolhido
    gravar_texto(&arquivo, r#"{"tema": "dark", "lugares_paineis": "janela"}"#);
    assert_eq!(abrir(&arquivo).lugares(), LugaresPaineis::default());
}
