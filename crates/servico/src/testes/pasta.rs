//! A leitura da pasta (`pasta.rs`) com pastas temporárias de verdade: o que um caminho é, abrir, listar, fechar
//! e registrar. O atributo oculto vem do `attrib +h` do Windows e os atalhos de pasta, do `mklink /J` (juncao,
//! que dispensa privilégio).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use comum::arquivos::{Listagem, Natureza, TipoDeArquivo};
use prefs::{TipoRecente, chave_caminho};
use rusqlite::Connection;

use super::{Pasta, ambiente, erro_com, exclusivo, prefs_em};
use crate::Servico;
use crate::pasta::{Limites, varrer};

/// Uma pasta temporária com o serviço e a pasta `dados` (as preferências ficam fora dela).
struct Cenario {
    servico: Servico,
    dados: PathBuf,
    _pasta: Pasta,
}

fn cenario(rotulo: &str) -> Cenario {
    let pasta = Pasta::nova(rotulo);
    let dados = pasta.join("dados");
    fs::create_dir(&dados).unwrap();
    let servico = Servico::novo(prefs_em(&pasta));
    Cenario { servico, dados, _pasta: pasta }
}

impl Cenario {
    fn caminho(&self, nome: &str) -> PathBuf {
        self.dados.join(nome)
    }

    /// Cria (ou troca) o arquivo com estes bytes.
    fn escrever(&self, nome: &str, bytes: &[u8]) -> PathBuf {
        let destino = self.caminho(nome);
        fs::write(&destino, bytes).unwrap();
        destino
    }

    fn pasta(&self, nome: &str) -> PathBuf {
        let destino = self.caminho(nome);
        fs::create_dir_all(&destino).unwrap();
        destino
    }

    /// Um banco SQLite de verdade (com o cabeçalho e mais de 100 bytes).
    fn banco(&self, nome: &str) -> PathBuf {
        let destino = self.caminho(nome);
        Connection::open(&destino).unwrap().execute_batch("CREATE TABLE t (a INTEGER)").unwrap();
        destino
    }

    fn listar(&self) -> Listagem {
        self.servico.listar_pasta(&self.dados, None).unwrap()
    }

    /// O tipo com que o caminho está nos recentes.
    fn tipo_recente(&self, caminho: &Path) -> Option<TipoRecente> {
        let chave = chave_caminho(caminho);
        self.servico.prefs().recentes().into_iter().find(|r| chave_caminho(&r.caminho) == chave).map(|r| r.tipo)
    }
}

fn nomes(l: &Listagem) -> Vec<&str> {
    l.entradas.iter().map(|e| e.nome.as_str()).collect()
}

/// Bytes que não são SQLite, com o tamanho pedido.
fn lixo(tamanho: usize) -> Vec<u8> {
    b"isto nao e um banco SQLite, e so um arquivo qualquer ".iter().copied().cycle().take(tamanho).collect()
}

#[cfg(windows)]
fn comando_ok(programa: &str, argumentos: &[&Path], antes: &[&str]) {
    let ok = std::process::Command::new(programa).args(antes).args(argumentos).status().is_ok_and(|s| s.success());
    assert!(ok, "{programa} {antes:?} {argumentos:?} deveria ter funcionado");
}

/// `attrib +h`: o atributo oculto do Windows.
#[cfg(windows)]
fn esconder(caminho: &Path) {
    comando_ok("attrib", &[caminho], &["+h"]);
}

/// Uma juncao `link` que aponta para `destino` (`mklink /J`).
#[cfg(windows)]
fn juncao(link: &Path, destino: &Path) {
    comando_ok("cmd", &[link, destino], &["/C", "mklink", "/J"]);
}

// ---- alvo_do_caminho ----

#[test]
fn alvo_de_pasta_banco_e_dados() {
    let c = cenario("alvo-tipos");
    let pasta = c.pasta("sub");
    let banco = c.banco("vendas.db");
    let alvos = [
        (pasta, Natureza::Pasta),
        (banco, Natureza::Banco),
        (c.escrever("clientes.csv", b"id;nome\n"), Natureza::Dados(TipoDeArquivo::Csv)),
        (c.escrever("PLANILHA.XLSX", b"PK\x03\x04"), Natureza::Dados(TipoDeArquivo::Xlsx)),
        (c.escrever("dados.json", b"[]"), Natureza::Dados(TipoDeArquivo::Json)),
        (c.escrever("notas.txt", b"oi"), Natureza::Dados(TipoDeArquivo::Texto)),
        (c.escrever("leia-me.md", b""), Natureza::Dados(TipoDeArquivo::Texto)),
    ];
    for (caminho, esperado) in alvos {
        assert_eq!(c.servico.alvo_do_caminho(&caminho).unwrap(), esperado, "{}", caminho.display());
    }
    // o caminho digitado é limpo como os demais (as aspas do "Copiar como caminho")
    let com_aspas = PathBuf::from(format!("  \"{}\"  ", c.caminho("vendas.db").display()));
    assert_eq!(c.servico.alvo_do_caminho(&com_aspas).unwrap(), Natureza::Banco);
    // inexistente: 404; vazio: 400
    let sumiu = c.caminho("sumiu.db");
    let e = erro_com(c.servico.alvo_do_caminho(&sumiu), 404);
    assert_eq!(e.mensagem, format!("O arquivo não existe (ou foi movido): {}.", sumiu.display()));
    assert_eq!(
        erro_com(c.servico.alvo_do_caminho(Path::new("  ")), 400).mensagem,
        "Informe o caminho da pasta ou do arquivo."
    );
}

#[test]
fn alvo_do_banco_pelo_cabecalho_e_nao_pela_extensao() {
    let c = cenario("alvo-cabecalho");
    // um SQLite com outro nome ou sem extensão é banco
    let verdadeiro = c.banco("base.db");
    let bytes = fs::read(&verdadeiro).unwrap();
    for nome in ["copia.dat", "sem_extensao", "antigo.bak", "banco.DB3"] {
        c.escrever(nome, &bytes);
        assert_eq!(c.servico.alvo_do_caminho(&c.caminho(nome)).unwrap(), Natureza::Banco, "{nome}");
    }
    // um `.db` que não é SQLite (o Thumbs.db do Windows), os -wal e os pequenos demais não são
    for (nome, conteudo) in [
        ("Thumbs.db", lixo(24_576)),
        ("falso.db", lixo(100)),
        ("base.db-wal", lixo(32_992)),
        ("minusculo.db", lixo(50)),
    ] {
        let alvo = c.escrever(nome, &conteudo);
        let e = erro_com(c.servico.alvo_do_caminho(&alvo), 400);
        assert_eq!(e.mensagem, format!("O DataBox ainda não abre este tipo de arquivo: {nome}."), "{nome}");
    }
}

#[test]
fn alvo_de_arquivo_vazio_com_extensao_de_banco() {
    let c = cenario("alvo-vazio");
    for nome in ["novo.db", "novo.sqlite", "novo.SQLITE3", "novo.db3", "novo.s3db", "novo.sl3"] {
        let vazio = c.escrever(nome, b"");
        assert_eq!(c.servico.alvo_do_caminho(&vazio).unwrap(), Natureza::Banco, "{nome}");
    }
    let vazio_sem_banco = c.escrever("novo.dat", b"");
    assert_eq!(
        erro_com(c.servico.alvo_do_caminho(&vazio_sem_banco), 400).mensagem,
        "O DataBox ainda não abre este tipo de arquivo: novo.dat."
    );
    let vazio_de_dados = c.escrever("vazio.csv", b"");
    assert_eq!(c.servico.alvo_do_caminho(&vazio_de_dados).unwrap(), Natureza::Dados(TipoDeArquivo::Csv));
}

#[test]
fn alvo_de_tipo_desconhecido_diz_que_nao_abre() {
    let c = cenario("alvo-desconhecido");
    for (nome, conteudo) in [
        ("relatorio.pdf", b"%PDF-1.4".to_vec()),
        ("grande.pdf", lixo(5_000)),
        ("programa.exe", lixo(300)),
        ("antigo.xls", lixo(100)),
        ("consulta.sql", b"select 1".to_vec()),
    ] {
        let alvo = c.escrever(nome, &conteudo);
        let e = erro_com(c.servico.alvo_do_caminho(&alvo), 400);
        assert_eq!(e.mensagem, format!("O DataBox ainda não abre este tipo de arquivo: {nome}."));
    }
}

// ---- listar_pasta ----

/// A releitura da volta do foco compara a listagem nova com a de antes: ler uma subpasta (o NTFS atualiza a data
/// dela na pasta de cima só quando ela é aberta) não pode fazer a pasta de cima parecer outra.
#[test]
fn listar_de_novo_sem_mudanca_da_a_mesma_listagem_depois_de_ler_a_subpasta() {
    let c = cenario("listar-igual");
    let sub = c.pasta("sub");
    c.banco("vendas.db");
    c.escrever("notas.txt", b"oi");
    fs::write(sub.join("dentro.csv"), b"a\n1\n").unwrap();
    let antes = c.listar();
    c.servico.listar_pasta(&sub, None).unwrap();
    let depois = c.servico.listar_pasta(&c.dados, Some(&antes)).unwrap();
    assert_eq!(antes, depois);
}

#[test]
fn listar_ordena_pastas_bancos_e_dados() {
    let c = cenario("listar-ordem");
    for nome in ["Zeta", "alfa", "dados10", "dados2"] {
        c.pasta(nome);
    }
    for nome in ["B10.db", "b2.db", "Ab.db"] {
        c.banco(nome);
    }
    for nome in ["notas.txt", "Clientes.csv", "a.json"] {
        c.escrever(nome, b"x");
    }
    let l = c.listar();
    assert_eq!(
        nomes(&l),
        ["alfa", "dados2", "dados10", "Zeta", "Ab.db", "b2.db", "B10.db", "a.json", "Clientes.csv", "notas.txt"]
    );
    assert_eq!(l.pasta, c.dados);
    assert_eq!((l.ocultas, l.a_mais, l.parcial), (0, 0, false));
    for e in &l.entradas {
        assert_eq!(e.caminho, c.caminho(&e.nome));
        assert_eq!(e.chave, chave_caminho(&e.caminho), "a chave do caminho, calculada na leitura");
        assert_eq!(e.modificado.is_some(), e.natureza != Natureza::Pasta, "só arquivo tem data: {}", e.nome);
        assert_eq!(e.tamanho.is_some(), e.natureza != Natureza::Pasta, "só arquivo tem tamanho: {}", e.nome);
    }
    let csv = l.entrada(&chave_caminho(c.caminho("clientes.csv"))).unwrap();
    assert_eq!((csv.natureza, csv.tamanho), (Natureza::Dados(TipoDeArquivo::Csv), Some(1)));
    let banco = l.entrada(&chave_caminho(c.caminho("ab.db"))).unwrap();
    assert_eq!(banco.natureza, Natureza::Banco);
    assert_eq!(banco.tamanho, Some(fs::metadata(c.caminho("Ab.db")).unwrap().len()));
}

#[test]
fn listar_esconde_ocultos_til_dolar_e_pastas_de_ferramentas() {
    let c = cenario("listar-ocultos");
    for nome in [".git", ".svn", ".hg", ".vscode", ".idea", "__pycache__", "node_modules", ".venv", ".GIT2"] {
        c.pasta(nome);
    }
    c.pasta("vendas");
    c.pasta(".github");
    c.escrever("~$planilha.xlsx", b"bloqueio do Office");
    c.escrever("dados.csv", b"x");
    // os nomes de ferramenta só escondem pasta: um arquivo com o nome fica
    c.escrever("node_modules.txt", b"x");
    let mut ocultas = 1; // ~$planilha.xlsx
    #[cfg(windows)]
    {
        let escondido = c.escrever("escondido.csv", b"x");
        esconder(&escondido);
        let pasta_escondida = c.pasta("pasta_escondida");
        esconder(&pasta_escondida);
        ocultas += 1; // a pasta oculta sai, mas só os arquivos entram na conta
    }
    let l = c.listar();
    assert_eq!(nomes(&l), [".GIT2", ".github", "vendas", "dados.csv", "node_modules.txt"]);
    assert_eq!(l.ocultas, ocultas);
}

#[test]
fn listar_mostra_pasta_com_ponto() {
    let c = cenario("listar-ponto");
    c.pasta(".coletor_noticias");
    c.pasta(".github");
    c.banco(".coletor.db");
    c.escrever(".notas.txt", b"x");
    c.escrever(".env", b"SEGREDO=1");
    let l = c.listar();
    assert_eq!(nomes(&l), [".coletor_noticias", ".github", ".coletor.db", ".notas.txt"]);
    assert_eq!(l.ocultas, 1, "o .env é de outro tipo, não oculto por causa do ponto");
}

#[test]
fn listar_conta_as_ocultas_de_outros_tipos() {
    let c = cenario("listar-outros");
    c.escrever("relatorio.pdf", &lixo(30));
    c.escrever("Thumbs.db", &lixo(24_576));
    c.escrever("programa.exe", &lixo(200));
    c.escrever("pequeno.db", &lixo(50));
    c.escrever("base.db-wal", &lixo(200));
    c.escrever("sem_nada", b"");
    c.escrever("ok.csv", b"x");
    c.banco("b.db");
    let l = c.listar();
    assert_eq!(nomes(&l), ["b.db", "ok.csv"]);
    assert_eq!((l.ocultas, l.a_mais, l.parcial), (6, 0, false));
}

#[test]
fn listar_corta_em_500_e_diz_quantos_faltam() {
    let c = cenario("listar-corte");
    for i in 0..12 {
        c.escrever(&format!("d{i:02}.csv"), b"x");
    }
    // o limite reduzido pelo teste
    let reduzido = Limites { por_pasta: 5, ..Limites::PADRAO };
    let v = varrer(&c.dados, None, reduzido).unwrap();
    assert_eq!(nomes(&v.listagem), ["d00.csv", "d01.csv", "d02.csv", "d03.csv", "d04.csv"]);
    assert_eq!((v.listagem.a_mais, v.listagem.parcial), (7, false), "cortar no limite não é leitura parcial");
    // o limite de verdade
    for i in 12..520 {
        c.escrever(&format!("d{i:03}.csv"), b"x");
    }
    let l = c.listar();
    assert_eq!((l.entradas.len(), l.a_mais, l.parcial), (500, 20, false));
    assert!(l.entradas.iter().all(|e| !e.chave.is_empty()));
}

#[test]
fn listar_para_no_limite_de_leitura_e_marca_parcial() {
    let c = cenario("listar-leitura");
    for i in 0..10 {
        c.escrever(&format!("d{i}.csv"), b"x");
    }
    let v = varrer(&c.dados, None, Limites { leitura: 4, ..Limites::PADRAO }).unwrap();
    assert!(v.listagem.parcial);
    assert_eq!(v.listagem.entradas.len(), 4);
    // exatamente no limite não é parcial: só quando ainda havia mais
    let justo = varrer(&c.dados, None, Limites { leitura: 10, ..Limites::PADRAO }).unwrap();
    assert!(!justo.listagem.parcial);
    assert_eq!(justo.listagem.entradas.len(), 10);
}

#[test]
fn listar_para_no_prazo_e_marca_parcial() {
    let c = cenario("listar-prazo");
    for i in 0..10 {
        c.escrever(&format!("d{i}.csv"), b"x");
    }
    c.banco("b.db");
    // prazo zero: a varredura para na primeira entrada e diz que pode faltar coisa
    let v = varrer(&c.dados, None, Limites { prazo: Duration::ZERO, ..Limites::PADRAO }).unwrap();
    assert!(v.listagem.parcial);
    assert!(v.listagem.entradas.len() < 11);
    assert_eq!(v.cabecalhos_lidos, 0);
    // com o prazo de verdade, tudo
    let inteira = varrer(&c.dados, None, Limites::PADRAO).unwrap();
    assert!(!inteira.listagem.parcial);
    assert_eq!(inteira.listagem.entradas.len(), 11);
}

#[test]
fn listar_limita_os_cabecalhos_e_le_primeiro_os_de_extensao_de_banco() {
    let c = cenario("listar-cabecalhos");
    for nome in ["a.dat", "b.dat", "c.dat", "d.dat", "e.dat", "f.dat"] {
        c.escrever(nome, &lixo(120));
    }
    c.banco("z.db"); // o último na ordem dos nomes, mas o primeiro a ser lido
    let limitada = varrer(&c.dados, None, Limites { cabecalhos: 3, ..Limites::PADRAO }).unwrap();
    assert_eq!(limitada.cabecalhos_lidos, 3);
    assert!(limitada.listagem.parcial, "ficaram arquivos sem o cabeçalho lido");
    assert_eq!(nomes(&limitada.listagem), ["z.db"]);
    assert_eq!(limitada.listagem.ocultas, 2, "a.dat e b.dat foram lidos e não são banco");
    // sem o corte, todos são lidos e nenhum é parcial
    let inteira = varrer(&c.dados, None, Limites::PADRAO).unwrap();
    assert_eq!((inteira.cabecalhos_lidos, inteira.listagem.parcial, inteira.listagem.ocultas), (7, false, 6));
}

#[test]
fn listar_reaproveita_o_cabecalho_se_nada_mudou() {
    let c = cenario("listar-reaproveita");
    let banco = c.banco("b.db");
    let primeira = varrer(&c.dados, None, Limites::PADRAO).unwrap();
    assert_eq!((nomes(&primeira.listagem), primeira.cabecalhos_lidos), (vec!["b.db"], 1));

    // o conteúdo muda, mas o tamanho e a data ficam: o cabeçalho não é lido de novo
    let tamanho = fs::metadata(&banco).unwrap().len();
    let data = fs::metadata(&banco).unwrap().modified().unwrap();
    fs::write(&banco, lixo(tamanho as usize)).unwrap();
    let datar_em = |arquivo: &Path, quando: SystemTime| {
        fs::OpenOptions::new().write(true).open(arquivo).unwrap().set_modified(quando).unwrap();
    };
    let datar = |quando: SystemTime| datar_em(&banco, quando);
    datar(data);
    let reaproveitada = varrer(&c.dados, Some(&primeira.listagem), Limites::PADRAO).unwrap();
    assert_eq!((nomes(&reaproveitada.listagem), reaproveitada.cabecalhos_lidos), (vec!["b.db"], 0));
    // pela API pública também
    assert_eq!(nomes(&c.servico.listar_pasta(&c.dados, Some(&primeira.listagem)).unwrap()), ["b.db"]);
    // sem o anterior, o cabeçalho é lido e o arquivo já não é banco
    let sem_anterior = varrer(&c.dados, None, Limites::PADRAO).unwrap();
    assert_eq!(
        (sem_anterior.listagem.entradas.len(), sem_anterior.cabecalhos_lidos, sem_anterior.listagem.ocultas),
        (0, 1, 1)
    );
    // a data mudou: lê de novo
    datar(data + Duration::from_secs(5));
    let relida = varrer(&c.dados, Some(&primeira.listagem), Limites::PADRAO).unwrap();
    assert_eq!((relida.listagem.entradas.len(), relida.cabecalhos_lidos), (0, 1));
    // o tamanho mudou: também
    datar(data);
    fs::write(&banco, lixo(tamanho as usize + 1)).unwrap();
    datar(data);
    let maior = varrer(&c.dados, Some(&primeira.listagem), Limites::PADRAO).unwrap();
    assert_eq!((maior.listagem.entradas.len(), maior.cabecalhos_lidos), (0, 1));
    // a leitura anterior de outra pasta não vale, mesmo com o mesmo nome, o mesmo tamanho e a mesma data
    let falso = c.pasta("outra").join("b.db");
    fs::write(&falso, lixo(tamanho as usize)).unwrap();
    datar_em(&falso, data);
    let de_outra = varrer(falso.parent().unwrap(), Some(&primeira.listagem), Limites::PADRAO).unwrap();
    assert_eq!((de_outra.listagem.entradas.len(), de_outra.cabecalhos_lidos), (0, 1));
}

#[cfg(windows)]
#[test]
fn listar_segue_atalho_de_pasta_e_ignora_o_quebrado() {
    let c = cenario("listar-atalho");
    let fora = Pasta::nova("listar-atalho-destino");
    fs::write(fora.join("interno.csv"), b"x").unwrap();
    fs::write(fora.join("outro.txt"), b"x").unwrap();
    let atalho = c.caminho("projetos");
    juncao(&atalho, &fora.caminho);
    // um atalho cujo destino sumiu
    let sumido = Pasta::nova("listar-atalho-sumido");
    let quebrado = c.caminho("quebrado");
    juncao(&quebrado, &sumido.caminho);
    fs::remove_dir(&sumido.caminho).unwrap();
    c.banco("b.db");

    let l = c.listar();
    assert_eq!(nomes(&l), ["projetos", "b.db"], "o atalho vira pasta e o quebrado sai");
    assert_eq!(l.entradas[0].natureza, Natureza::Pasta);
    assert_eq!(l.entradas[0].caminho, atalho);
    // abrir o atalho lê a pasta de destino
    let dentro = c.servico.listar_pasta(&atalho, None).unwrap();
    assert_eq!(nomes(&dentro), ["interno.csv", "outro.txt"]);
    assert_eq!(dentro.entradas[0].caminho, atalho.join("interno.csv"));
    assert_eq!(c.servico.alvo_do_caminho(&atalho).unwrap(), Natureza::Pasta);
    // as junções saem antes de a pasta temporária ir embora (não se segue o destino ao apagar)
    fs::remove_dir(&atalho).unwrap();
    fs::remove_dir(&quebrado).unwrap();
}

#[cfg(windows)]
#[test]
fn listar_atalho_que_volta_para_cima_nao_faz_laco() {
    let c = cenario("listar-laco");
    let sub = c.pasta("sub");
    let volta = sub.join("volta");
    juncao(&volta, &c.dados); // sub\volta -> dados
    c.escrever("a.csv", b"x");
    let raiz = c.listar();
    assert_eq!(nomes(&raiz), ["sub", "a.csv"]);
    // cada leitura é um nível só: andar em círculo é a pessoa abrindo de novo, e cada passo termina na hora
    let mut atual = c.dados.clone();
    for _ in 0..4 {
        let inicio = Instant::now();
        let l = c.servico.listar_pasta(&atual, None).unwrap();
        assert!(inicio.elapsed() < Duration::from_secs(2));
        assert_eq!(nomes(&l), ["sub", "a.csv"]);
        let sub_aberta = c.servico.listar_pasta(&l.entradas[0].caminho, None).unwrap();
        assert_eq!(nomes(&sub_aberta), ["volta"]);
        // as chaves do mesmo nome em níveis diferentes são diferentes (o id de cada nó leva o caminho inteiro)
        assert_ne!(sub_aberta.entradas[0].chave, l.entradas[0].chave);
        atual = sub_aberta.entradas[0].caminho.clone();
    }
    fs::remove_dir(&volta).unwrap();
}

#[test]
fn listar_pasta_inexistente_ou_arquivo_falha_com_frase() {
    let c = cenario("listar-erros");
    let sumiu = c.caminho("sumiu");
    let e = erro_com(c.servico.listar_pasta(&sumiu, None), 404);
    assert_eq!(e.mensagem, format!("Arquivo ou pasta não encontrado: {}.", sumiu.display()));
    let arquivo = c.escrever("a.csv", b"x");
    let e = erro_com(c.servico.listar_pasta(&arquivo, None), 400);
    assert_eq!(e.mensagem, format!("O caminho não é uma pasta: {}.", arquivo.display()));
    erro_com(c.servico.listar_pasta(Path::new(""), None), 400);
}

// ---- abrir, fechar e registrar ----

#[test]
fn abrir_pasta_entra_nos_recentes_e_grava_a_pasta_aberta() {
    let c = cenario("abrir-pasta");
    c.banco("b.db");
    c.escrever("a.csv", b"x");
    assert_eq!(c.servico.prefs().pasta_aberta(), None);
    let l = c.servico.abrir_pasta(&c.dados).unwrap();
    assert_eq!(nomes(&l), ["b.db", "a.csv"]);
    assert_eq!(l.pasta, c.dados);
    assert_eq!(c.tipo_recente(&c.dados), Some(TipoRecente::Pasta));
    let guardada = c.servico.prefs().pasta_aberta().expect("a pasta aberta fica guardada");
    assert_eq!(chave_caminho(&guardada), chave_caminho(&c.dados));
    // com aspas e espaços nas pontas, o mesmo
    c.servico.fechar_pasta();
    let digitado = PathBuf::from(format!(" \"{}\" ", c.dados.display()));
    assert_eq!(nomes(&c.servico.abrir_pasta(&digitado).unwrap()), ["b.db", "a.csv"]);
    assert_eq!(c.servico.prefs().recentes().iter().filter(|r| r.tipo == TipoRecente::Pasta).count(), 1);
    // abrir de novo não duplica nem tira o recente
    c.servico.abrir_pasta(&c.dados).unwrap();
    assert_eq!(c.servico.prefs().recentes().iter().filter(|r| r.tipo == TipoRecente::Pasta).count(), 1);
}

#[test]
fn abrir_pasta_inexistente_404_e_arquivo_400() {
    let c = cenario("abrir-pasta-erros");
    let sumiu = c.caminho("sumiu");
    let e = erro_com(c.servico.abrir_pasta(&sumiu), 404);
    assert_eq!(e.mensagem, format!("A pasta não existe (ou foi movida): {}.", sumiu.display()));
    let arquivo = c.escrever("a.csv", b"x");
    let e = erro_com(c.servico.abrir_pasta(&arquivo), 400);
    assert_eq!(e.mensagem, format!("O caminho não é uma pasta: {}.", arquivo.display()));
    assert_eq!(erro_com(c.servico.abrir_pasta(Path::new("")), 400).mensagem, "Informe o caminho da pasta.");
    // nada foi gravado
    assert!(c.servico.prefs().recentes().is_empty());
    assert_eq!(c.servico.prefs().pasta_aberta(), None);
}

#[test]
fn fechar_pasta_apaga_a_chave_e_mantem_o_recente() {
    let c = cenario("fechar-pasta");
    c.servico.abrir_pasta(&c.dados).unwrap();
    assert!(c.servico.prefs().pasta_aberta().is_some());
    c.servico.fechar_pasta();
    assert_eq!(c.servico.prefs().pasta_aberta(), None);
    assert_eq!(c.tipo_recente(&c.dados), Some(TipoRecente::Pasta), "os recentes ficam");
    // fechar sem pasta aberta não faz mal
    c.servico.fechar_pasta();
    assert_eq!(c.servico.prefs().pasta_aberta(), None);
}

#[test]
fn registrar_arquivo_entra_nos_recentes_como_arquivo() {
    let c = cenario("registrar-arquivo");
    let csv = c.escrever("clientes.csv", b"id;nome\n1;Ana\n");
    let info = c.servico.registrar_arquivo(&csv).unwrap();
    assert_eq!(info.caminho, csv);
    assert_eq!(info.nome, "clientes.csv");
    assert_eq!(info.tipo, TipoDeArquivo::Csv);
    assert_eq!(info.tamanho, 14);
    assert_eq!(info.modificado_em, fs::metadata(&csv).unwrap().modified().ok());
    assert_eq!(c.tipo_recente(&csv), Some(TipoRecente::Arquivo));
    // pasta, banco, tipo desconhecido e inexistente não são arquivo de dados
    let banco = c.banco("b.db");
    assert_eq!(
        erro_com(c.servico.registrar_arquivo(&banco), 400).mensagem,
        format!("O caminho não é um arquivo de dados: {}.", banco.display())
    );
    erro_com(c.servico.registrar_arquivo(&c.dados), 400);
    let pdf = c.escrever("relatorio.pdf", b"%PDF");
    assert_eq!(
        erro_com(c.servico.registrar_arquivo(&pdf), 400).mensagem,
        "O DataBox ainda não abre este tipo de arquivo: relatorio.pdf."
    );
    erro_com(c.servico.registrar_arquivo(&c.caminho("sumiu.csv")), 404);
    for fora in [&banco, &c.dados, &pdf] {
        assert_eq!(c.tipo_recente(fora), None, "{} não entra nos recentes", fora.display());
    }
}

#[test]
fn abrir_banco_continua_como_banco_nos_recentes() {
    let c = cenario("abrir-banco-recente");
    let banco = c.banco("b.db");
    let info = c.servico.abrir_banco(&banco, false, false).unwrap();
    assert_eq!(c.tipo_recente(&banco), Some(TipoRecente::Banco));
    // a pasta e o arquivo convivem com ele nos recentes
    c.servico.abrir_pasta(&c.dados).unwrap();
    let csv = c.escrever("a.csv", b"x");
    c.servico.registrar_arquivo(&csv).unwrap();
    assert_eq!(c.tipo_recente(&banco), Some(TipoRecente::Banco));
    assert_eq!(c.tipo_recente(&c.dados), Some(TipoRecente::Pasta));
    assert_eq!(c.tipo_recente(&csv), Some(TipoRecente::Arquivo));
    c.servico.fechar_banco(&info.id, true).unwrap();
}

// ---- o banco aberto e o desempenho ----

#[test]
fn listar_a_pasta_do_banco_aberto_nao_espera_a_conexao() {
    let _vez = exclusivo();
    let amb = ambiente("pasta-banco-aberto");
    let banco = amb.servico.atual().expect("o banco de exemplo está aberto");
    let (preso_tx, preso_rx) = mpsc::channel::<()>();
    let (solta_tx, solta_rx) = mpsc::channel::<()>();
    thread::scope(|s| {
        let segurando = std::sync::Arc::clone(&banco);
        let tarefa = s.spawn(move || {
            segurando
                .com_conexao(|_| {
                    preso_tx.send(()).unwrap();
                    let _ = solta_rx.recv_timeout(Duration::from_secs(20));
                    Ok(())
                })
                .unwrap();
        });
        preso_rx.recv_timeout(Duration::from_secs(10)).expect("a conexão foi presa");
        let inicio = Instant::now();
        let l = amb.servico.listar_pasta(&amb.pasta.caminho, None).unwrap();
        let exemplo = l.entradas.iter().find(|e| e.nome == "exemplo.db").expect("o banco aberto aparece na lista");
        assert_eq!(exemplo.natureza, Natureza::Banco);
        assert_eq!(amb.servico.alvo_do_caminho(&amb.exemplo).unwrap(), Natureza::Banco);
        let aberta = amb.servico.abrir_pasta(&amb.pasta.caminho).unwrap();
        assert!(aberta.entrada(&exemplo.chave).is_some());
        let csv = amb.pasta.join("a.csv");
        fs::write(&csv, "x").unwrap();
        amb.servico.registrar_arquivo(&csv).unwrap();
        amb.servico.fechar_pasta();
        let demorou = inicio.elapsed();
        solta_tx.send(()).unwrap();
        tarefa.join().unwrap();
        assert!(demorou < Duration::from_secs(2), "esperou a conexão do banco: {demorou:?}");
    });
}

/// Cria `n_csv` arquivos `.csv` e `n_dat` arquivos `.dat` (de 200 bytes, que não são banco) numa pasta nova e diz
/// quanto levou (com o antivírus da máquina, criar arquivo custa milissegundos cada um).
fn pasta_com_muitos(rotulo: &str, n_csv: usize, n_dat: usize) -> (Pasta, Duration) {
    let pasta = Pasta::nova(rotulo);
    let inicio = Instant::now();
    for i in 0..n_csv {
        fs::write(pasta.join(&format!("arquivo{i:05}.csv")), b"a;b\n1;2\n").unwrap();
    }
    let dat = lixo(200);
    for i in 0..n_dat {
        fs::write(pasta.join(&format!("copia{i:05}.dat")), &dat).unwrap();
    }
    (pasta, inicio.elapsed())
}

/// Uma pasta média, com os dois caminhos da varredura (o que nem abre e o que lê o cabeçalho): o teto é folgado
/// porque a leitura do cabeçalho de arquivo recém-criado paga o antivírus. O que a varredura acha é conferido sem o
/// prazo de 5 s (com a máquina carregada de compilações, o prazo cortaria e a contagem mudaria); o tempo vale pela
/// melhor de três leituras, para um pico de carga não reprovar a leitura que é rápida.
#[test]
fn listar_pasta_media_com_cabecalhos_e_rapido() {
    let _vez = exclusivo();
    let (pasta, _) = pasta_com_muitos("media", 1_000, 100);
    let sem_prazo = Limites { prazo: Duration::from_secs(120), ..Limites::PADRAO };
    let mut tempos = Vec::new();
    for _ in 0..3 {
        let inicio = Instant::now();
        let v = varrer(&pasta.caminho, None, sem_prazo).unwrap();
        tempos.push(inicio.elapsed());
        assert_eq!(v.cabecalhos_lidos, 100);
        assert_eq!((v.listagem.entradas.len(), v.listagem.a_mais, v.listagem.ocultas), (500, 500, 100));
        assert!(!v.listagem.parcial);
        if tempos.last().is_some_and(|t| *t < Duration::from_secs(3)) {
            return;
        }
    }
    panic!("listar 1.100 arquivos levou {tempos:?}");
}

/// O desempenho do plano: 10.000 arquivos reconhecidos pela extensão (nenhum cabeçalho lido) listados em cerca de
/// 1 s. Fica `#[ignore]` porque só criar os arquivos leva uns 20 s com antivírus ativo. Roda à mão:
/// `.\cargo.ps1 test -p servico listar_dez_mil '--' --ignored --nocapture`. Depois mede a leitura dos cabeçalhos
/// (2.000 `.dat`, fria e com o antivírus já satisfeito), que não tem teto: o prazo de 5 s é que corta.
#[test]
#[ignore = "cria 12.000 arquivos (leva tempo com o antivírus); rode à mão"]
fn listar_dez_mil_arquivos_em_cerca_de_um_segundo() {
    let _vez = exclusivo();
    let (pasta, criacao) = pasta_com_muitos("dez-mil", 10_000, 0);
    let inicio = Instant::now();
    let v = varrer(&pasta.caminho, None, Limites::PADRAO).unwrap();
    let tempo = inicio.elapsed();
    eprintln!("10.000 arquivos reconhecidos: criar {criacao:?}, listar {tempo:?}");
    assert_eq!(v.cabecalhos_lidos, 0);
    assert_eq!((v.listagem.entradas.len(), v.listagem.a_mais, v.listagem.ocultas), (500, 9_500, 0));
    assert!(!v.listagem.parcial);
    assert!(tempo < Duration::from_millis(1_500), "listar 10.000 arquivos levou {tempo:?}");

    let dat = lixo(200);
    for i in 0..2_000 {
        fs::write(pasta.join(&format!("copia{i:05}.dat")), &dat).unwrap();
    }
    for rotulo in ["fria", "morna"] {
        let inicio = Instant::now();
        let v = varrer(&pasta.caminho, None, Limites::PADRAO).unwrap();
        eprintln!(
            "mais 2.000 .dat ({rotulo}): listar {:?}, {} cabeçalhos lidos, parcial {}",
            inicio.elapsed(),
            v.cabecalhos_lidos,
            v.listagem.parcial
        );
        assert!(inicio.elapsed() < PRAZO_COM_FOLGA);
    }
}

/// O prazo de 5 s da varredura mais a folga de uma leitura de cabeçalho em andamento.
const PRAZO_COM_FOLGA: Duration = Duration::from_secs(6);

/// 20.500 arquivos: passa das 20.000 entradas lidas e marca `parcial`. À mão, como o de cima.
#[test]
#[ignore = "cria 20.500 arquivos (leva tempo com o antivírus); rode à mão"]
fn listar_vinte_mil_arquivos_bate_no_limite_de_leitura() {
    let _vez = exclusivo();
    let (pasta, criacao) = pasta_com_muitos("vinte-mil", 20_500, 0);
    let inicio = Instant::now();
    let v = varrer(&pasta.caminho, None, Limites::PADRAO).unwrap();
    eprintln!("20.500 arquivos: criar {criacao:?}, listar {:?}", inicio.elapsed());
    assert!(v.listagem.parcial, "passou das 20.000 entradas lidas");
    assert_eq!((v.listagem.entradas.len(), v.listagem.a_mais), (500, 19_500));
}
