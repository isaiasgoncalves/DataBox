//! Testes do CSV com arquivos de verdade: abrir, detectar, paginar, editar, salvar e conferir os bytes.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

use comum::{Codigo, ErroBanco, Resultado};

use super::*;
use crate::testes_apoio::Pasta;

// ---------------------------------------------------------------------------------------------------
// Apoio
// ---------------------------------------------------------------------------------------------------

fn sem_progresso(_: u64, _: u64) -> Resultado<()> {
    Ok(())
}

fn abrir(pasta: &Pasta, nome: &str, bytes: &[u8]) -> Tabela {
    let caminho = pasta.arquivo(nome, bytes);
    Tabela::abrir(&caminho, &OpcoesAbertura::default()).unwrap()
}

fn tudo(t: &mut Tabela) -> Vec<Vec<String>> {
    let n = usize::try_from(t.linhas()).unwrap();
    t.pagina(0, n + 5).unwrap()
}

fn l(linhas: &[&[&str]]) -> Vec<Vec<String>> {
    linhas.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect()
}

fn salvar(t: &mut Tabela) -> ResumoSalvar {
    t.salvar(&mut sem_progresso).unwrap()
}

fn bytes(t: &Tabela) -> Vec<u8> {
    fs::read(t.caminho()).unwrap()
}

/// As opções que reabrem o arquivo exatamente como a tabela o lê.
fn opcoes_de(f: &Formato) -> OpcoesAbertura {
    OpcoesAbertura {
        codificacao: Some(f.codificacao),
        separador: Some(f.separador),
        fim_de_linha: Some(f.fim_de_linha),
        aspas: Some(f.aspas),
        cabecalho: Some(f.cabecalho),
        decimal: Some(f.decimal),
    }
}

/// O índice que a gravação monta tem de ser igual ao de uma leitura nova do arquivo gravado.
fn confere_o_indice(t: &Tabela) {
    let novo = Leitor::abrir(t.caminho(), &opcoes_de(t.formato())).unwrap();
    assert_eq!(novo.indice(), t.leitor().indice(), "o índice da gravação difere do de uma leitura nova");
    assert_eq!(novo.formato(), t.formato());
    assert_eq!(novo.nomes(), t.leitor().nomes());
}

const EXPORT: &str = "\u{FEFF}codigo;produto;preco\r\n004718;Caderno universitário;24,90\r\n004725;\"Caneta; gel\";4,50\r\n004731;Mochila;129,90\r\n";

// ---------------------------------------------------------------------------------------------------
// Abrir e detectar
// ---------------------------------------------------------------------------------------------------

#[test]
fn abre_o_export_do_databox() {
    let pasta = Pasta::nova("export");
    let mut t = abrir(&pasta, "estoque.csv", EXPORT.as_bytes());
    let f = t.formato().clone();
    assert_eq!(f.codificacao, Codificacao::Utf8ComBom);
    assert_eq!((f.separador, f.decimal, f.fim_de_linha, f.aspas), (';', ',', FimDeLinha::Crlf, EstiloAspas::Minimas));
    assert!(f.cabecalho && f.termina_em_quebra);
    assert_eq!((t.linhas(), t.colunas()), (3, 3));
    assert_eq!(t.nomes(), ["codigo", "produto", "preco"]);
    assert_eq!(
        tudo(&mut t),
        l(&[
            &["004718", "Caderno universitário", "24,90"],
            &["004725", "Caneta; gel", "4,50"],
            &["004731", "Mochila", "129,90"]
        ])
    );
    assert_eq!(t.leitor().tamanho(), EXPORT.len() as u64);
    assert_eq!(t.celula(1, 1).unwrap(), "Caneta; gel");
    assert!(!t.alterada());
}

#[test]
fn utf8_sem_bom_e_windows_1252() {
    let pasta = Pasta::nova("codificacoes");
    let t = abrir(&pasta, "a.csv", "nome;cidade\r\nJosé;São Paulo\r\n".as_bytes());
    assert_eq!(t.formato().codificacao, Codificacao::Utf8);
    let t = abrir(&pasta, "so_ascii.csv", b"nome;cidade\r\nAna;Recife\r\n");
    assert_eq!(t.formato().codificacao, Codificacao::Utf8, "só ASCII é UTF-8");
    let mut t = abrir(&pasta, "b.csv", b"nome;cidade\r\nJos\xE9;S\xE3o Paulo\r\nMaria;Bel\xE9m\r\n");
    assert_eq!(t.formato().codificacao, Codificacao::Windows1252);
    assert_eq!(tudo(&mut t), l(&[&["José", "São Paulo"], &["Maria", "Belém"]]));
    // pedir outra codificação vale mais que a detectada
    let caminho = pasta.join("b.csv");
    let opcoes = OpcoesAbertura { codificacao: Some(Codificacao::Utf8), ..OpcoesAbertura::default() };
    let mut u = Tabela::abrir(&caminho, &opcoes).unwrap();
    assert_eq!(tudo(&mut u)[0][0], "Jos\u{FFFD}");
}

#[test]
fn utf8_invalido_so_no_fim_do_arquivo_grande_e_windows_1252() {
    // a amostra do começo é UTF-8 válido, mas o arquivo todo não é
    let pasta = Pasta::nova("invalido_no_fim");
    let mut texto = String::from("id;nome\r\n");
    for i in 0..30_000 {
        texto.push_str(&format!("{i};linha com acento ção\r\n"));
    }
    let mut b = texto.into_bytes();
    assert!(b.len() > TAMANHO_DA_AMOSTRA);
    b.extend_from_slice(b"99999;fim \xE7\xE3o\r\n");
    let mut t = abrir(&pasta, "a.csv", &b);
    assert_eq!(t.formato().codificacao, Codificacao::Windows1252, "um byte inválido em qualquer lugar decide");
    assert_eq!(t.celula(30_000, 1).unwrap(), "fim ção");
}

#[test]
fn bom_com_codificacao_forcada() {
    let pasta = Pasta::nova("bom_forcado");
    let caminho = pasta.arquivo("a.csv", "\u{FEFF}x;y\r\n1;2\r\n".as_bytes());
    let cp = OpcoesAbertura { codificacao: Some(Codificacao::Windows1252), ..OpcoesAbertura::default() };
    let t = Tabela::abrir(&caminho, &cp).unwrap();
    assert_eq!(t.nomes()[0], "ï»¿x", "lido como Windows-1252, o BOM aparece como caracteres");
    let sem = OpcoesAbertura { codificacao: Some(Codificacao::Utf8), ..OpcoesAbertura::default() };
    let t = Tabela::abrir(&caminho, &sem).unwrap();
    assert_eq!(t.nomes(), ["x", "y"]);
    assert_eq!(t.formato().codificacao, Codificacao::Utf8);
}

#[test]
fn utf8_escolhido_num_arquivo_que_nao_e_recusa_salvar() {
    let pasta = Pasta::nova("utf8_escolhido");
    let original: &[u8] = b"nome;cidade\r\nJos\xE9;S\xE3o Paulo\r\nMaria;Bel\xE9m\r\n";
    let caminho = pasta.arquivo("a.csv", original);
    assert!(abrir(&pasta, "b.csv", "nome\r\nJosé\r\n".as_bytes()).leitor().utf8_valido());
    // lido como UTF-8 (a Codificação das Propriedades), as células trazem U+FFFD: gravar estragaria as outras
    let opcoes = OpcoesAbertura { codificacao: Some(Codificacao::Utf8), ..OpcoesAbertura::default() };
    let mut t = Tabela::abrir(&caminho, &opcoes).unwrap();
    assert!(!t.leitor().utf8_valido());
    t.editar_celula(1, 0, "Marta").unwrap();
    let e = t.salvar(&mut sem_progresso).expect_err("recusa gravar");
    assert_eq!(e.mensagem, MSG_NAO_E_UTF8);
    assert_eq!(fs::read(&caminho).unwrap(), original, "nada foi gravado");
    assert!(t.alterada(), "as alterações continuam");
    let outro = pasta.join("c.csv");
    assert!(t.salvar_como(&outro, &Formato::padrao(), &mut sem_progresso).is_err(), "nem em outro arquivo");
    assert!(!outro.exists());
    pasta.sem_temporarios();
    // lido como Windows-1252 (o detectado), grava, e as células que ninguém mexeu ficam como estavam
    let mut t = Tabela::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
    t.editar_celula(1, 0, "Marta").unwrap();
    salvar(&mut t);
    assert_eq!(fs::read(&caminho).unwrap(), b"nome;cidade\r\nJos\xE9;S\xE3o Paulo\r\nMarta;Bel\xE9m\r\n");
}

#[test]
fn salvar_nao_devolve_erro_depois_de_trocar_o_arquivo() {
    let pasta = Pasta::nova("sem_indice");
    let mut t = abrir(&pasta, "a.csv", b"a;b\r\n1;2\r\n3;4\r\n");
    t.editar_celula(0, 1, "x").unwrap();
    t.excluir_linhas(&[1]).unwrap();
    // o leitor com o índice da gravação falha: a tabela lê o arquivo gravado de novo
    tabela::FALHAR_COM_INDICE.with(|f| f.set(true));
    let r = t.salvar(&mut sem_progresso);
    tabela::FALHAR_COM_INDICE.with(|f| f.set(false));
    r.expect("o arquivo foi gravado: nada de erro");
    assert_eq!(bytes(&t), b"a;b\r\n1;x\r\n");
    assert!(!t.alterada() && t.alteracoes() == 0, "as alterações recomeçam do arquivo gravado");
    assert_eq!(tudo(&mut t), l(&[&["1", "x"]]));
    confere_o_indice(&t);
    pasta.sem_temporarios();
}

#[test]
fn cada_separador() {
    let pasta = Pasta::nova("separadores");
    for (sep, c) in [(';', ";"), (',', ","), ('\t', "\t"), ('|', "|")] {
        let texto = format!("a{c}b{c}c\r\n1{c}2{c}3\r\n4{c}5{c}6\r\n");
        let mut t = abrir(&pasta, "x.csv", texto.as_bytes());
        assert_eq!(t.formato().separador, sep);
        assert_eq!(tudo(&mut t), l(&[&["1", "2", "3"], &["4", "5", "6"]]));
    }
}

#[test]
fn opcoes_forcadas_valem_mais_que_a_deteccao() {
    let pasta = Pasta::nova("forcadas");
    let caminho = pasta.arquivo("a.csv", b"a;b\r\n1;2\r\n");
    let opcoes = OpcoesAbertura {
        separador: Some(','),
        cabecalho: Some(false),
        fim_de_linha: Some(FimDeLinha::Lf),
        decimal: Some('.'),
        aspas: Some(EstiloAspas::Todas),
        ..OpcoesAbertura::default()
    };
    let t = Tabela::abrir(&caminho, &opcoes).unwrap();
    let f = t.formato();
    assert_eq!(
        (f.separador, f.cabecalho, f.fim_de_linha, f.decimal, f.aspas),
        (',', false, FimDeLinha::Lf, '.', EstiloAspas::Todas)
    );
    assert_eq!((t.linhas(), t.colunas()), (2, 1));
    assert!(Tabela::abrir(&caminho, &OpcoesAbertura { separador: Some('"'), ..OpcoesAbertura::default() }).is_err());
    assert!(Tabela::abrir(&caminho, &OpcoesAbertura { decimal: Some(';'), ..OpcoesAbertura::default() }).is_err());
}

#[test]
fn quebras_de_linha_e_aspas_dentro_das_celulas() {
    let pasta = Pasta::nova("aspas");
    let texto = "id;obs\r\n1;\"linha 1\r\nlinha 2\"\r\n2;\"diz \"\"oi\"\"\"\r\n3;\"a;b\"\r\n";
    let mut t = abrir(&pasta, "a.csv", texto.as_bytes());
    assert_eq!(t.linhas(), 3);
    assert_eq!(tudo(&mut t), l(&[&["1", "linha 1\r\nlinha 2"], &["2", "diz \"oi\""], &["3", "a;b"]]));
}

#[test]
fn registros_irregulares() {
    let pasta = Pasta::nova("irregulares");
    let mut t = abrir(&pasta, "a.csv", b"a;b;c\r\n1;2\r\n3;4;5;6\r\n");
    assert_eq!(t.colunas(), 4);
    assert_eq!(t.nomes(), ["a", "b", "c", "coluna_4"]);
    assert_eq!(t.leitor().indice().irregulares, 2);
    assert_eq!(tudo(&mut t), l(&[&["1", "2", "", ""], &["3", "4", "5", "6"]]));
}

#[test]
fn sem_cabecalho_e_alternar() {
    let pasta = Pasta::nova("sem_cabecalho");
    let mut t = abrir(&pasta, "a.csv", b"1;2\r\n3;4\r\n");
    assert!(!t.formato().cabecalho);
    assert_eq!(t.nomes(), ["coluna_1", "coluna_2"]);
    assert_eq!(t.linhas(), 2);
    t.definir_cabecalho(true).unwrap();
    assert_eq!(t.nomes(), ["1", "2"]);
    assert_eq!(tudo(&mut t), l(&[&["3", "4"]]));
    t.editar_celula(0, 0, "x").unwrap();
    assert_eq!(t.definir_cabecalho(false).unwrap_err().status, 409, "com alterações pendentes não troca");
    assert!(t.desfazer());
    t.definir_cabecalho(false).unwrap();
    assert_eq!(t.linhas(), 2);
}

#[test]
fn arquivos_pequenos_e_de_borda() {
    let pasta = Pasta::nova("bordas");
    let mut t = abrir(&pasta, "vazio.csv", b"");
    assert_eq!((t.linhas(), t.colunas()), (0, 1));
    assert_eq!(t.nomes(), ["coluna_1"]);
    assert!(tudo(&mut t).is_empty());
    let t = abrir(&pasta, "so_bom.csv", &[0xEF, 0xBB, 0xBF]);
    assert_eq!((t.linhas(), t.formato().codificacao), (0, Codificacao::Utf8ComBom));
    let mut t = abrir(&pasta, "cabecalho.csv", b"nome;cidade\r\n");
    assert_eq!((t.linhas(), t.colunas()), (0, 2));
    assert_eq!(t.nomes(), ["nome", "cidade"]);
    assert!(tudo(&mut t).is_empty());
    let mut t = abrir(&pasta, "sem_quebra.csv", b"nome;cidade\r\nAna;Recife");
    assert!(!t.formato().termina_em_quebra);
    assert_eq!(tudo(&mut t), l(&[&["Ana", "Recife"]]));
    let mut t = abrir(&pasta, "uma_coluna.csv", b"nome\r\nAna\r\nBia\r\n");
    assert_eq!((t.colunas(), t.formato().separador), (1, ';'));
    assert_eq!(tudo(&mut t), l(&[&["Ana"], &["Bia"]]));
    let mut t = abrir(&pasta, "em_branco.csv", b"a;b\r\n1;2\r\n\r\n3;4\r\n");
    assert_eq!(tudo(&mut t), l(&[&["1", "2"], &["", ""], &["3", "4"]]), "linha em branco é uma linha vazia");
}

#[test]
fn recusa_o_que_nao_e_csv_de_texto() {
    let pasta = Pasta::nova("recusas");
    let utf16 = pasta.arquivo("u16.csv", &[0xFF, 0xFE, b'a', 0, b';', 0, b'b', 0]);
    let e = Tabela::abrir(&utf16, &OpcoesAbertura::default()).unwrap_err();
    assert!(e.mensagem.contains("UTF-16"), "{e}");
    let be = pasta.arquivo("u16be.csv", &[0xFE, 0xFF, 0, b'a']);
    assert!(Tabela::abrir(&be, &OpcoesAbertura::default()).unwrap_err().mensagem.contains("UTF-16"));
    let binario = pasta.arquivo("bin.csv", b"a;b\r\n1;\0;3\r\n");
    assert!(Tabela::abrir(&binario, &OpcoesAbertura::default()).unwrap_err().mensagem.contains("não parece de texto"));
    let e = Tabela::abrir(&pasta.join("nao_existe.csv"), &OpcoesAbertura::default()).unwrap_err();
    assert_eq!(e.status, 404);
    assert!(e.mensagem.starts_with("Não foi possível ler o arquivo."), "{e}");
    let e = Tabela::abrir(pasta.caminho(), &OpcoesAbertura::default()).unwrap_err();
    assert!(!e.mensagem.is_empty());
}

// ---------------------------------------------------------------------------------------------------
// Páginas e índice
// ---------------------------------------------------------------------------------------------------

fn arquivo_grande(linhas: u64) -> (Vec<u8>, Vec<Vec<String>>) {
    let mut texto = String::from("id;texto;obs\r\n");
    let mut esperado = Vec::new();
    for i in 0..linhas {
        let obs = if i % 7 == 3 { format!("linha {i}\r\ncom quebra") } else { format!("obs {i}") };
        let quote = |s: &str| if s.contains('\n') { format!("\"{s}\"") } else { s.to_string() };
        texto.push_str(&format!("{i};texto {i};{}\r\n", quote(&obs)));
        esperado.push(vec![i.to_string(), format!("texto {i}"), obs]);
    }
    (texto.into_bytes(), esperado)
}

#[test]
fn paginas_em_qualquer_posicao_e_tamanho() {
    let pasta = Pasta::nova("paginas");
    let (texto, esperado) = arquivo_grande(1000);
    let caminho = pasta.arquivo("g.csv", &texto);
    let mut leitor = Leitor::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
    assert_eq!((leitor.linhas(), leitor.colunas()), (1000, 3));
    assert_eq!(leitor.indice().pontos().len(), 1001usize.div_ceil(64));
    assert_eq!(leitor.linhas_de(0, 1000).unwrap(), esperado);
    for (inicio, n) in [
        (0u64, 1usize),
        (0, 64),
        (1, 63),
        (63, 2),
        (64, 1),
        (64, 64),
        (65, 130),
        (500, 100),
        (990, 10),
        (999, 1),
        (997, 50),
    ] {
        let de = usize::try_from(inicio).unwrap();
        let certo: Vec<Vec<String>> = esperado[de..(de + n).min(1000)].to_vec();
        assert_eq!(leitor.linhas_de(inicio, n).unwrap(), certo, "inicio {inicio}, n {n}");
    }
    assert!(leitor.linhas_de(1000, 5).unwrap().is_empty());
    assert!(leitor.linhas_de(5000, 5).unwrap().is_empty());
    assert!(leitor.linhas_de(0, 0).unwrap().is_empty());
    assert_eq!(leitor.linha(3).unwrap(), Some(esperado[3].clone()));
    assert_eq!(leitor.linha(1000).unwrap(), None);
    assert_eq!(leitor.registros(0, 1).unwrap(), l(&[&["id", "texto", "obs"]]), "o registro 0 é o cabeçalho");
    // lendo de trás para a frente, sem depender da posição anterior do cursor
    for i in (0..1000u64).rev().step_by(37) {
        assert_eq!(leitor.linha(i).unwrap().unwrap(), esperado[usize::try_from(i).unwrap()]);
    }
}

#[test]
fn arquivo_alterado_por_fora_nao_e_lido_as_cegas() {
    let pasta = Pasta::nova("mudou");
    let (texto, _) = arquivo_grande(200);
    let caminho = pasta.arquivo("g.csv", &texto);
    let mut t = Tabela::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
    assert!(!t.leitor().mudou_no_disco());
    fs::OpenOptions::new().append(true).open(&caminho).unwrap().write_all(b"999;novo;x\r\n").unwrap();
    assert!(t.leitor().mudou_no_disco());
    let e = t.pagina(0, 10).unwrap_err();
    assert_eq!(e.status, 409);
    assert!(e.mensagem.contains("alterado por outro programa"));
}

#[test]
fn salvar_recusa_o_arquivo_que_mudou_por_fora() {
    let pasta = Pasta::nova("mudou_salvar");
    let caminho = pasta.arquivo("a.csv", b"a;b\r\n1;2\r\n");
    let mut t = Tabela::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
    t.editar_celula(0, 0, "9").unwrap();
    fs::write(&caminho, b"a;b\r\n1;2\r\n3;4\r\n").unwrap();
    let e = t.salvar(&mut sem_progresso).unwrap_err();
    assert_eq!(e.status, 409);
    assert_eq!(fs::read(&caminho).unwrap(), b"a;b\r\n1;2\r\n3;4\r\n", "o que o outro programa gravou não foi pisado");
    pasta.sem_temporarios();
    assert!(t.alterada());
}

#[test]
fn varrer_e_colunas_de() {
    let pasta = Pasta::nova("varrer");
    let (texto, esperado) = arquivo_grande(300);
    let caminho = pasta.arquivo("g.csv", &texto);
    let mut leitor = Leitor::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
    let colunas = leitor.colunas_de_sem_progresso(&[2, 0, 2]).unwrap();
    assert_eq!(colunas.len(), 3);
    assert_eq!(colunas[0], esperado.iter().map(|r| r[2].clone()).collect::<Vec<_>>());
    assert_eq!(colunas[1], esperado.iter().map(|r| r[0].clone()).collect::<Vec<_>>());
    assert_eq!(colunas[0], colunas[2]);
    let fora = leitor.colunas_de_sem_progresso(&[7]).unwrap();
    assert!(fora[0].iter().all(String::is_empty) && fora[0].len() == 300, "coluna que não existe: vazia");
    // parar no meio
    let mut vistas = Vec::new();
    leitor
        .varrer(&mut sem_progresso, |i, vista| {
            vistas.push((i, vista.campo(0), vista.n_campos(), vista.todos().len()));
            Ok(i < 4)
        })
        .unwrap();
    assert_eq!(vistas.len(), 5);
    assert_eq!(vistas[4], (4, "4".to_string(), 3, 3));
    // um erro do chamador sobe como veio
    let e =
        leitor.varrer(&mut sem_progresso, |_, _| Err(ErroBanco::novo("pare").codigo(Codigo::Cancelado))).unwrap_err();
    assert!(e.tem_codigo(Codigo::Cancelado));
    // o progresso também cancela
    let mut cancela = |_: u64, _: u64| Err(ErroBanco::novo("cancelado").status(409));
    let e = leitor.colunas_de(&[0], &mut cancela).unwrap_err();
    assert_eq!(e.status, 409);
}

#[test]
fn leitor_reaberto_tem_cursor_proprio() {
    let pasta = Pasta::nova("reaberto");
    let (texto, esperado) = arquivo_grande(200);
    let caminho = pasta.arquivo("g.csv", &texto);
    let mut a = Leitor::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
    let mut b = a.reabrir_para_leitura().unwrap();
    assert_eq!(a.indice(), b.indice());
    assert_eq!(b.linha(150).unwrap().unwrap(), esperado[150]);
    assert_eq!(a.linha(3).unwrap().unwrap(), esperado[3]);
    assert_eq!(b.linha(3).unwrap().unwrap(), esperado[3]);
}

// ---------------------------------------------------------------------------------------------------
// Editar e salvar
// ---------------------------------------------------------------------------------------------------

const ASPAS_SOBRANDO: &str = "\u{FEFF}codigo;produto;preco\r\n004718;\"Caderno\";24,90\r\n004725;\"Caneta; gel\";4,50\r\n004731;Mochila;129,90\r\n";

#[test]
fn uma_celula_regrava_so_a_linha_dela() {
    let pasta = Pasta::nova("uma_celula");
    let mut t = abrir(&pasta, "a.csv", ASPAS_SOBRANDO.as_bytes());
    assert!(t.editar_celula(2, 2, "129,95").unwrap());
    assert!(t.alterada());
    let r = salvar(&mut t);
    assert_eq!((r.linhas, r.linhas_regravadas), (3, 1));
    assert_eq!(
        bytes(&t),
        "\u{FEFF}codigo;produto;preco\r\n004718;\"Caderno\";24,90\r\n004725;\"Caneta; gel\";4,50\r\n004731;Mochila;129,95\r\n".as_bytes(),
        "as aspas que sobram nas outras linhas ficam como estavam"
    );
    assert!(!t.alterada() && !t.pode_desfazer());
    pasta.sem_temporarios();
    confere_o_indice(&t);
}

#[test]
fn a_linha_editada_segue_o_estilo_e_as_outras_ficam_como_estavam() {
    let pasta = Pasta::nova("estilo_da_linha");
    let mut t = abrir(&pasta, "a.csv", ASPAS_SOBRANDO.as_bytes());
    t.editar_celula(0, 1, "Caderno 96 fls").unwrap();
    t.editar_celula(1, 1, "Caneta \"gel\"").unwrap();
    salvar(&mut t);
    assert_eq!(
        String::from_utf8(bytes(&t)).unwrap(),
        "\u{FEFF}codigo;produto;preco\r\n004718;Caderno 96 fls;24,90\r\n004725;\"Caneta \"\"gel\"\"\";4,50\r\n004731;Mochila;129,90\r\n"
    );
    assert_eq!(tudo(&mut t)[1][1], "Caneta \"gel\"");
    confere_o_indice(&t);
}

#[test]
fn inserir_excluir_e_renomear() {
    let pasta = Pasta::nova("estrutura");
    let cabecalho = "\u{FEFF}codigo;produto;preco\r\n";
    let r0 = "004718;\"Caderno\";24,90\r\n";
    let r1 = "004725;\"Caneta; gel\";4,50\r\n";
    let r2 = "004731;Mochila;129,90\r\n";
    let original = format!("{cabecalho}{r0}{r1}{r2}");

    // inserir uma linha vazia entre as duas primeiras
    let mut t = abrir(&pasta, "a.csv", original.as_bytes());
    t.inserir_linha(1).unwrap();
    assert_eq!(tudo(&mut t)[1], ["", "", ""]);
    salvar(&mut t);
    assert_eq!(String::from_utf8(bytes(&t)).unwrap(), format!("{cabecalho}{r0};;\r\n{r1}{r2}"));
    confere_o_indice(&t);

    // excluir a primeira e a última
    let mut t = abrir(&pasta, "b.csv", original.as_bytes());
    t.excluir_linhas(&[0, 2]).unwrap();
    assert_eq!(t.linhas(), 1);
    let r = salvar(&mut t);
    assert_eq!((r.linhas, r.linhas_regravadas), (1, 0));
    assert_eq!(String::from_utf8(bytes(&t)).unwrap(), format!("{cabecalho}{r1}"));
    confere_o_indice(&t);

    // excluir tudo: sobra o cabeçalho
    let mut t = abrir(&pasta, "c.csv", original.as_bytes());
    t.excluir_linhas(&[0, 1, 2]).unwrap();
    salvar(&mut t);
    assert_eq!(String::from_utf8(bytes(&t)).unwrap(), cabecalho);
    assert_eq!(t.linhas(), 0);
    confere_o_indice(&t);

    // inserir uma coluna no meio: todas as linhas são regravadas
    let mut t = abrir(&pasta, "d.csv", original.as_bytes());
    t.inserir_coluna(1, "extra").unwrap();
    t.editar_celula(2, 1, "x y").unwrap();
    let r = salvar(&mut t);
    assert_eq!(r.linhas_regravadas, 3);
    assert_eq!(
        String::from_utf8(bytes(&t)).unwrap(),
        "\u{FEFF}codigo;extra;produto;preco\r\n004718;;Caderno;24,90\r\n004725;;\"Caneta; gel\";4,50\r\n004731;x y;Mochila;129,90\r\n"
    );
    assert_eq!(t.nomes(), ["codigo", "extra", "produto", "preco"]);
    confere_o_indice(&t);

    // excluir uma coluna
    let mut t = abrir(&pasta, "e.csv", original.as_bytes());
    t.excluir_colunas(&[1]).unwrap();
    salvar(&mut t);
    assert_eq!(
        String::from_utf8(bytes(&t)).unwrap(),
        "\u{FEFF}codigo;preco\r\n004718;24,90\r\n004725;4,50\r\n004731;129,90\r\n"
    );
    confere_o_indice(&t);

    // renomear: só o cabeçalho é regravado, as linhas ficam como estavam
    let mut t = abrir(&pasta, "f.csv", original.as_bytes());
    t.renomear_coluna(2, "valor").unwrap();
    let r = salvar(&mut t);
    assert_eq!(r.linhas_regravadas, 0);
    assert_eq!(String::from_utf8(bytes(&t)).unwrap(), format!("\u{FEFF}codigo;produto;valor\r\n{r0}{r1}{r2}"));
    confere_o_indice(&t);
}

#[test]
fn nome_de_coluna_com_separador_vai_entre_aspas() {
    let pasta = Pasta::nova("nome_citado");
    let mut t = abrir(&pasta, "a.csv", b"a;b\r\n1;2\r\n");
    t.renomear_coluna(0, "preço; unitário").unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), "\"preço; unitário\";b\r\n1;2\r\n".as_bytes());
    assert_eq!(t.nomes()[0], "preço; unitário");
}

#[test]
fn o_formato_do_arquivo_e_mantido_ao_salvar() {
    let pasta = Pasta::nova("formato_mantido");
    // LF, vírgula, sem BOM, sem quebra no fim
    let mut t = abrir(&pasta, "a.csv", b"a,b\nx,\"y\"\nz,w");
    assert_eq!(t.formato().fim_de_linha, FimDeLinha::Lf);
    assert!(!t.formato().termina_em_quebra);
    t.editar_celula(0, 1, "q,r").unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), b"a,b\nx,\"q,r\"\nz,w");
    t.editar_celula(1, 0, "zz").unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), b"a,b\nx,\"q,r\"\nzz,w", "continua sem quebra no fim");
    t.inserir_linha(2).unwrap();
    salvar(&mut t);
    assert_eq!(
        bytes(&t),
        b"a,b\nx,\"q,r\"\nzz,w\n,",
        "a última linha antiga ganhou a quebra; o arquivo segue sem quebra no fim"
    );
    confere_o_indice(&t);

    // tabulação e CR
    let mut t = abrir(&pasta, "b.csv", b"a\tb\rx\ty\rz\tw\r");
    assert_eq!((t.formato().separador, t.formato().fim_de_linha), ('\t', FimDeLinha::Cr));
    t.editar_celula(0, 0, "x\ty").unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), b"a\tb\r\"x\ty\"\ty\rz\tw\r");
    confere_o_indice(&t);

    // sem BOM e com BOM continuam como estavam
    let mut t = abrir(&pasta, "c.csv", "ação;b\r\n1;2\r\n".as_bytes());
    t.editar_celula(0, 0, "7").unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), "ação;b\r\n7;2\r\n".as_bytes());
    let mut t = abrir(&pasta, "d.csv", "\u{FEFF}ação;b\r\n1;2\r\n".as_bytes());
    t.editar_celula(0, 0, "7").unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), "\u{FEFF}ação;b\r\n7;2\r\n".as_bytes());
}

#[test]
fn estilo_de_aspas_todas_e_nao_numericos() {
    let pasta = Pasta::nova("aspas_estilo");
    let mut t = abrir(&pasta, "a.csv", "\"nome\";\"cidade\"\r\n\"Ana\";\"Recife\"\r\n\"Bia\";\"Natal\"\r\n".as_bytes());
    assert_eq!(t.formato().aspas, EstiloAspas::Todas);
    t.editar_celula(0, 1, "Olinda").unwrap();
    t.inserir_linha(2).unwrap();
    salvar(&mut t);
    assert_eq!(
        String::from_utf8(bytes(&t)).unwrap(),
        "\"nome\";\"cidade\"\r\n\"Ana\";\"Olinda\"\r\n\"Bia\";\"Natal\"\r\n\"\";\"\"\r\n"
    );
    let mut t =
        abrir(&pasta, "b.csv", "\"nome\";\"idade\"\r\n\"Ana\";30\r\n\"Bia\";41\r\n\"Caio\";52,5\r\n".as_bytes());
    assert_eq!(t.formato().aspas, EstiloAspas::NaoNumericos);
    t.editar_celula(0, 1, "31").unwrap();
    t.editar_celula(1, 0, "Beatriz").unwrap();
    salvar(&mut t);
    assert_eq!(
        String::from_utf8(bytes(&t)).unwrap(),
        "\"nome\";\"idade\"\r\n\"Ana\";31\r\n\"Beatriz\";41\r\n\"Caio\";52,5\r\n"
    );
}

#[test]
fn windows_1252_mantem_a_codificacao() {
    let pasta = Pasta::nova("cp1252");
    let mut t = abrir(&pasta, "a.csv", b"nome;cidade\r\nJos\xE9;S\xE3o Paulo\r\nAna;Bel\xE9m\r\n");
    assert_eq!(t.formato().codificacao, Codificacao::Windows1252);
    t.editar_celula(1, 1, "Maceió").unwrap();
    t.editar_celula(0, 0, "Zé €5").unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), b"nome;cidade\r\nZ\xE9 \x805;S\xE3o Paulo\r\nAna;Macei\xF3\r\n");
    assert_eq!(t.formato().codificacao, Codificacao::Windows1252);
    assert_eq!(tudo(&mut t), l(&[&["Zé €5", "São Paulo"], &["Ana", "Maceió"]]));
    confere_o_indice(&t);
}

#[test]
fn caractere_fora_do_windows_1252_nao_estraga_o_arquivo() {
    let pasta = Pasta::nova("cp1252_erro");
    let original = b"nome;cidade\r\nJos\xE9;S\xE3o Paulo\r\n".to_vec();
    let mut t = abrir(&pasta, "a.csv", &original);
    t.editar_celula(0, 1, "東京").unwrap();
    let e = t.salvar(&mut sem_progresso).unwrap_err();
    assert!(e.mensagem.contains("Windows-1252") && e.mensagem.contains("linha 1"), "{e}");
    assert_eq!(bytes(&t), original, "o arquivo ficou como estava");
    pasta.sem_temporarios();
    assert!(t.alterada(), "as alterações continuam lá");
    // trocando o caractere, salva
    t.editar_celula(0, 1, "Tokyo").unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), b"nome;cidade\r\nJos\xE9;Tokyo\r\n");
    // e o cabeçalho também
    t.renomear_coluna(0, "名前").unwrap();
    assert!(t.salvar(&mut sem_progresso).unwrap_err().mensagem.contains("Windows-1252"));
    pasta.sem_temporarios();
}

#[test]
fn quebras_de_linha_misturadas_sao_mantidas() {
    let pasta = Pasta::nova("misturadas");
    let mut t = abrir(&pasta, "a.csv", b"a;b\r\n1;2\n3;4\r\n5;6");
    assert_eq!(t.formato().fim_de_linha, FimDeLinha::Crlf);
    t.editar_celula(0, 1, "8").unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), b"a;b\r\n1;8\n3;4\r\n5;6");
    t.editar_celula(2, 0, "9").unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), b"a;b\r\n1;8\n3;4\r\n9;6");
}

#[test]
fn sem_alteracoes_salvar_nao_toca_no_arquivo() {
    let pasta = Pasta::nova("sem_mudanca");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    let antes = fs::metadata(t.caminho()).unwrap().modified().unwrap();
    let r = salvar(&mut t);
    assert_eq!((r.linhas, r.linhas_regravadas, r.bytes), (3, 0, EXPORT.len() as u64));
    assert_eq!(fs::metadata(t.caminho()).unwrap().modified().unwrap(), antes);
    // editar e desfazer volta a "sem alterações"
    t.editar_celula(0, 0, "x").unwrap();
    assert!(t.alterada());
    t.desfazer();
    assert!(!t.alterada());
    salvar(&mut t);
    assert_eq!(fs::metadata(t.caminho()).unwrap().modified().unwrap(), antes);
    pasta.sem_temporarios();
}

#[test]
fn editar_celula_so_conta_quando_muda() {
    let pasta = Pasta::nova("editar");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    assert!(!t.editar_celula(0, 1, "Caderno universitário").unwrap(), "mesmo texto: nada muda");
    assert!(!t.alterada());
    assert!(t.editar_celula(0, 1, "outro").unwrap());
    assert!(!t.editar_celula(0, 1, "outro").unwrap());
    assert_eq!(t.celula(0, 1).unwrap(), "outro");
    // voltar ao texto do arquivo apaga a alteração da célula
    assert!(t.editar_celula(0, 1, "Caderno universitário").unwrap());
    assert!(!t.edicoes().tem_celulas_alteradas());
    assert!(t.editar_celula(9, 0, "x").is_err());
    assert!(t.editar_celula(0, 9, "x").is_err());
    assert!(t.celula(9, 0).is_err() && t.celula(0, 9).is_err());
}

#[test]
fn desfazer_e_refazer_com_a_pagina_e_a_gravacao() {
    let pasta = Pasta::nova("desfazer");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    t.editar_celula(0, 0, "A").unwrap();
    t.inserir_linha(1).unwrap();
    t.excluir_linhas(&[0]).unwrap();
    t.renomear_coluna(0, "k").unwrap();
    assert_eq!(t.nomes()[0], "k");
    assert_eq!(t.linhas(), 3);
    assert!(t.desfazer() && t.desfazer() && t.desfazer() && t.desfazer());
    assert!(!t.desfazer() && !t.alterada());
    assert_eq!(tudo(&mut t)[0][0], "004718");
    assert!(t.refazer() && t.refazer());
    assert_eq!(t.linhas(), 4);
    assert_eq!(tudo(&mut t)[0][0], "A");
    salvar(&mut t);
    assert!(!t.pode_desfazer() && !t.pode_refazer(), "depois de salvar o histórico recomeça");
    assert_eq!(t.linhas(), 4);
}

#[test]
fn colar_coluna_e_um_passo_so() {
    let pasta = Pasta::nova("colar");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    t.inserir_coluna_com_valores(1, "regiao", &["Sul".into(), "".into(), "Norte".into()]).unwrap();
    assert_eq!(t.nomes(), ["codigo", "regiao", "produto", "preco"]);
    assert_eq!(tudo(&mut t)[0], ["004718", "Sul", "Caderno universitário", "24,90"]);
    assert_eq!(tudo(&mut t)[1][1], "");
    assert_eq!(t.edicoes().celulas().count(), 2);
    salvar(&mut t);
    assert_eq!(
        String::from_utf8(bytes(&t)).unwrap(),
        "\u{FEFF}codigo;regiao;produto;preco\r\n004718;Sul;Caderno universitário;24,90\r\n004725;;\"Caneta; gel\";4,50\r\n004731;Norte;Mochila;129,90\r\n"
    );
    let mut t = abrir(&pasta, "b.csv", EXPORT.as_bytes());
    t.inserir_coluna_com_valores(3, "x", &["1".into(), "2".into(), "3".into()]).unwrap();
    assert!(t.desfazer());
    assert_eq!(t.colunas(), 3);
    assert!(!t.alterada(), "uma desfeita só leva tudo");
    // por posição sobre uma coluna existente: None deixa como está
    t.preencher_coluna(2, &[Some("1,00".into()), None, Some("3,00".into()), Some("de sobra".into())]).unwrap();
    assert_eq!(tudo(&mut t).iter().map(|r| r[2].clone()).collect::<Vec<_>>(), ["1,00", "4,50", "3,00"]);
    assert!(t.desfazer());
    assert_eq!(tudo(&mut t)[0][2], "24,90");
}

#[test]
fn em_um_passo_desfaz_tudo_se_falhar() {
    let pasta = Pasta::nova("em_um_passo");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    let r: Resultado<()> = t.em_um_passo(|t| {
        t.inserir_coluna(0, "x")?;
        t.editar_celula(0, 0, "v")?;
        t.excluir_colunas(&[99])
    });
    assert!(r.is_err());
    assert!(!t.alterada());
    assert_eq!(t.colunas(), 3);
    assert!(!t.edicoes().grupo_aberto());
    // aninhado: tudo entra no grupo de fora
    t.em_um_passo(|t| {
        t.inserir_coluna(0, "a")?;
        t.em_um_passo(|t| t.inserir_coluna(0, "b"))?;
        assert!(t.edicoes().grupo_aberto());
        Ok(())
    })
    .unwrap();
    assert_eq!(t.colunas(), 5);
    assert!(t.desfazer() && !t.pode_desfazer());
    assert_eq!(t.colunas(), 3);
}

#[test]
fn salvar_como_converte_o_formato_e_passa_a_ser_o_arquivo_da_tabela() {
    let pasta = Pasta::nova("salvar_como");
    let original = b"nome;valor\r\nJos\xE9;1,5\r\nAna;\"2;5\"\r\n".to_vec();
    let mut t = abrir(&pasta, "a.csv", &original);
    assert_eq!(t.formato().codificacao, Codificacao::Windows1252);
    let destino = pasta.join("b.csv");
    let formato = Formato {
        codificacao: Codificacao::Utf8ComBom,
        separador: ',',
        fim_de_linha: FimDeLinha::Lf,
        ..t.formato().clone()
    };
    let r = t.salvar_como(&destino, &formato, &mut sem_progresso).unwrap();
    assert_eq!(r.linhas_regravadas, 2, "formato diferente: todas as linhas são regravadas");
    assert_eq!(bytes(&t), "\u{FEFF}nome,valor\nJosé,\"1,5\"\nAna,2;5\n".as_bytes());
    assert_eq!(fs::read(pasta.join("a.csv")).unwrap(), original, "o original não foi tocado");
    assert_eq!(t.caminho(), destino);
    assert_eq!(t.formato().codificacao, Codificacao::Utf8ComBom);
    assert_eq!(tudo(&mut t), l(&[&["José", "1,5"], &["Ana", "2;5"]]));
    confere_o_indice(&t);
    // converter para Windows-1252 de volta
    let de_volta = Formato { codificacao: Codificacao::Windows1252, ..formato.clone() };
    t.salvar_como(&pasta.join("c.csv"), &de_volta, &mut sem_progresso).unwrap();
    assert_eq!(bytes(&t), b"nome,valor\nJos\xE9,\"1,5\"\nAna,2;5\n");
}

#[test]
fn salvar_como_sem_alteracoes_copia_o_arquivo() {
    let pasta = Pasta::nova("copia");
    let mut t = abrir(&pasta, "a.csv", ASPAS_SOBRANDO.as_bytes());
    let destino = pasta.join("copia.csv");
    let formato = t.formato().clone();
    let r = t.salvar_como(&destino, &formato, &mut sem_progresso).unwrap();
    assert_eq!(r.linhas_regravadas, 0);
    assert_eq!(fs::read(&destino).unwrap(), ASPAS_SOBRANDO.as_bytes());
}

#[test]
fn cancelar_a_gravacao_deixa_o_arquivo_e_a_pasta_como_estavam() {
    let pasta = Pasta::nova("cancelar");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    t.editar_celula(0, 0, "x").unwrap();
    let mut cancela = |_: u64, _: u64| Err(ErroBanco::novo("Gravação interrompida.").codigo(Codigo::Cancelado));
    let e = t.salvar(&mut cancela).unwrap_err();
    assert!(e.tem_codigo(Codigo::Cancelado));
    assert_eq!(bytes(&t), EXPORT.as_bytes());
    pasta.sem_temporarios();
    assert!(t.alterada());
    assert_eq!(t.celula(0, 0).unwrap(), "x");
    // e depois salva normalmente
    salvar(&mut t);
    assert_eq!(t.celula(0, 0).unwrap(), "x");
}

#[test]
fn destino_que_nao_serve() {
    let pasta = Pasta::nova("destino_ruim");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    t.editar_celula(0, 0, "x").unwrap();
    let formato = t.formato().clone();
    let e = t.salvar_como(&pasta.join("nao_existe").join("b.csv"), &formato, &mut sem_progresso).unwrap_err();
    assert_eq!(e.status, 404);
    let e = t.salvar_como(pasta.caminho(), &formato, &mut sem_progresso).unwrap_err();
    assert!(e.mensagem.contains("pasta"));
    let ro = pasta.arquivo("ro.csv", b"x");
    let mut p = fs::metadata(&ro).unwrap().permissions();
    p.set_readonly(true);
    fs::set_permissions(&ro, p).unwrap();
    let e = t.salvar_como(&ro, &formato, &mut sem_progresso).unwrap_err();
    assert_eq!(e.status, 403);
    let mut p = fs::metadata(&ro).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    p.set_readonly(false);
    fs::set_permissions(&ro, p).unwrap();
    assert_eq!(bytes(&t), EXPORT.as_bytes());
    pasta.sem_temporarios();
    assert!(t.alterada());
}

#[test]
fn salva_por_cima_do_arquivo_com_outro_leitor_aberto() {
    let pasta = Pasta::nova("leitor_aberto");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    let mut outro = t.leitor().reabrir_para_leitura().unwrap();
    assert_eq!(outro.linha(0).unwrap().unwrap()[0], "004718");
    t.editar_celula(0, 0, "x").unwrap();
    salvar(&mut t);
    assert_eq!(t.celula(0, 0).unwrap(), "x");
    // o leitor velho, com o índice velho, percebe que o arquivo mudou
    assert!(outro.mudou_no_disco());
}

#[test]
fn reabrir_com_outras_opcoes() {
    let pasta = Pasta::nova("reabrir");
    let mut t = abrir(&pasta, "a.csv", b"a;b\r\n1;2\r\n");
    t.editar_celula(0, 0, "x").unwrap();
    let e = t
        .reabrir(&OpcoesAbertura { separador: Some(','), ..OpcoesAbertura::default() }, &mut sem_progresso)
        .unwrap_err();
    assert_eq!(e.status, 409);
    t.desfazer();
    t.reabrir(&OpcoesAbertura { separador: Some(','), ..OpcoesAbertura::default() }, &mut sem_progresso).unwrap();
    assert_eq!(t.colunas(), 1);
    assert_eq!(t.nomes(), ["a;b"]);
    t.reabrir(&OpcoesAbertura::default(), &mut sem_progresso).unwrap();
    assert_eq!(t.colunas(), 2);
}

#[test]
fn pagina_com_alteracoes_sem_salvar() {
    let pasta = Pasta::nova("virtual");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    t.excluir_linhas(&[1]).unwrap();
    t.inserir_linha(1).unwrap();
    t.editar_celula(1, 2, "novo").unwrap();
    t.inserir_coluna(0, "n").unwrap();
    t.editar_celula(0, 0, "primeiro").unwrap();
    assert_eq!(
        tudo(&mut t),
        l(&[
            &["primeiro", "004718", "Caderno universitário", "24,90"],
            &["", "", "", "novo"],
            &["", "004731", "Mochila", "129,90"]
        ])
    );
    assert_eq!(t.pagina(1, 1).unwrap(), l(&[&["", "", "", "novo"]]));
    assert_eq!(t.pagina(2, 5).unwrap().len(), 1);
    assert!(t.pagina(3, 5).unwrap().is_empty());
    let colunas = t.colunas_de(&[0, 3, 1], &mut sem_progresso).unwrap();
    assert_eq!(colunas[0], ["primeiro", "", ""]);
    assert_eq!(colunas[1], ["24,90", "novo", "129,90"]);
    assert_eq!(colunas[2], ["004718", "", "004731"]);
    assert!(t.colunas_de(&[9], &mut sem_progresso).is_err());
}

// ---------------------------------------------------------------------------------------------------
// Quantas alterações e quais células (o "1 alteração não salva" e o td.alt do editor)
// ---------------------------------------------------------------------------------------------------

#[test]
fn alteracoes_conta_os_passos_desde_o_ultimo_salvar() {
    let pasta = Pasta::nova("alteracoes");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    assert_eq!(t.alteracoes(), 0);
    t.editar_celula(0, 1, "outro").unwrap();
    assert_eq!(t.alteracoes(), 1);
    assert!(!t.editar_celula(0, 1, "outro").unwrap());
    assert_eq!(t.alteracoes(), 1, "o mesmo texto de novo não é alteração");
    t.editar_celula(0, 1, "e outro").unwrap();
    assert_eq!(t.alteracoes(), 2, "cada edição de célula é um passo");
    t.inserir_linha(1).unwrap();
    t.renomear_coluna(0, "k").unwrap();
    assert_eq!(t.alteracoes(), 4);
    // o que é um passo só no desfazer conta uma vez
    t.inserir_coluna_com_valores(1, "regiao", &["Sul".into(), "".into(), "Norte".into()]).unwrap();
    assert_eq!(t.alteracoes(), 5);
    t.em_um_passo(|t| {
        t.excluir_linhas(&[0])?;
        t.inserir_coluna(0, "z")?;
        t.editar_celula(0, 0, "v")
    })
    .unwrap();
    assert_eq!(t.alteracoes(), 6);
    assert_eq!(t.alteracoes() > 0, t.alterada());
    // uma operação que não muda nada não conta
    t.excluir_linhas(&[]).unwrap();
    t.renomear_coluna(0, "z").unwrap();
    assert_eq!(t.alteracoes(), 6);
    // uma que falha no meio de `em_um_passo` não deixa nada
    let r: Resultado<()> = t.em_um_passo(|t| {
        t.inserir_coluna(0, "w")?;
        t.excluir_colunas(&[99])
    });
    assert!(r.is_err());
    assert_eq!(t.alteracoes(), 6);
}

#[test]
fn alteracoes_volta_a_zero_depois_de_salvar() {
    let pasta = Pasta::nova("alteracoes_salvar");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    // salvar sem nada alterado não muda a conta
    salvar(&mut t);
    assert_eq!(t.alteracoes(), 0);
    t.editar_celula(0, 0, "A").unwrap();
    t.inserir_linha(0).unwrap();
    assert_eq!(t.alteracoes(), 2);
    salvar(&mut t);
    assert_eq!(t.alteracoes(), 0);
    assert!(!t.alterada() && !t.pode_desfazer() && !t.pode_refazer());
    t.editar_celula(1, 0, "B").unwrap();
    assert_eq!(t.alteracoes(), 1, "recomeça do 1 depois do Salvar");
    // salvar_como também vira o arquivo
    let destino = pasta.join("b.csv");
    let formato = t.formato().clone();
    t.salvar_como(&destino, &formato, &mut sem_progresso).unwrap();
    assert_eq!(t.alteracoes(), 0);
    // reabrir (sem alterações) e a abertura começam do zero
    t.reabrir(&OpcoesAbertura::default(), &mut sem_progresso).unwrap();
    assert_eq!(t.alteracoes(), 0);
    let nova = Tabela::abrir(&destino, &OpcoesAbertura::default()).unwrap();
    assert_eq!(nova.alteracoes(), 0);
}

#[test]
fn alteracoes_cai_no_desfazer_e_sobe_no_refazer() {
    let pasta = Pasta::nova("alteracoes_desfazer");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    t.editar_celula(0, 0, "A").unwrap();
    t.inserir_linha(1).unwrap();
    t.inserir_coluna_com_valores(0, "n", &["1".into(), "2".into()]).unwrap();
    assert_eq!(t.alteracoes(), 3);
    assert!(t.desfazer());
    assert_eq!(t.alteracoes(), 2, "desfazer o passo do grupo tira um, não dois");
    assert!(t.desfazer());
    assert_eq!(t.alteracoes(), 1);
    assert!(t.refazer());
    assert_eq!(t.alteracoes(), 2);
    // uma alteração nova depois de desfazer descarta o refazer e conta no lugar
    assert!(t.desfazer());
    t.editar_celula(1, 1, "x").unwrap();
    assert_eq!(t.alteracoes(), 2);
    assert!(!t.pode_refazer() && !t.refazer());
    assert!(t.desfazer() && t.desfazer());
    assert_eq!(t.alteracoes(), 0);
    assert!(!t.alterada(), "desfeito tudo, nada a salvar");
    assert!(!t.desfazer());
    assert_eq!(t.alteracoes(), 0);
    assert!(t.refazer() && t.refazer());
    assert_eq!(t.alteracoes(), 2);
}

#[test]
fn marcas_da_pagina_so_marca_celula_digitada() {
    let pasta = Pasta::nova("marcas");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    assert_eq!(t.marcas_da_pagina(0, 10), vec![vec![false; 3]; 3], "sem alterações, nada marcado");
    assert!(t.marcas_da_pagina(0, 0).is_empty());
    assert!(t.marcas_da_pagina(3, 5).is_empty() && t.marcas_da_pagina(99, 5).is_empty());
    t.editar_celula(1, 2, "9,99").unwrap();
    t.editar_celula(2, 0, "").unwrap(); // apagar o texto também é alteração
    assert_eq!(
        t.marcas_da_pagina(0, 10),
        vec![vec![false, false, false], vec![false, false, true], vec![true, false, false]]
    );
    // a janela pedida: só as linhas dela, com o mesmo tamanho de `pagina`
    assert_eq!(t.marcas_da_pagina(1, 1), vec![vec![false, false, true]]);
    assert_eq!(t.marcas_da_pagina(2, 5), vec![vec![true, false, false]]);
    assert_eq!(t.marcas_da_pagina(1, 5).len(), t.pagina(1, 5).unwrap().len());
    // voltar ao texto do arquivo apaga a marca
    t.editar_celula(1, 2, "4,50").unwrap();
    assert_eq!(t.marcas_da_pagina(1, 1), vec![vec![false; 3]]);
    // as marcas seguem o desfazer e o refazer
    assert!(t.desfazer());
    assert_eq!(t.marcas_da_pagina(1, 1), vec![vec![false, false, true]]);
    assert!(t.desfazer() && t.desfazer());
    assert_eq!(t.marcas_da_pagina(0, 10), vec![vec![false; 3]; 3]);
    assert!(t.refazer());
    assert_eq!(t.marcas_da_pagina(1, 1), vec![vec![false, false, true]]);
    // depois de salvar, o arquivo é o que está lá: sem marcas
    salvar(&mut t);
    assert_eq!(t.marcas_da_pagina(0, 10), vec![vec![false; 3]; 3]);
}

#[test]
fn marcas_da_pagina_ignora_coluna_nova_vazia() {
    let pasta = Pasta::nova("marcas_coluna");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    t.inserir_coluna(1, "nova").unwrap();
    assert_eq!(t.alteracoes(), 1, "a coluna nova conta como alteração");
    assert_eq!(t.marcas_da_pagina(0, 10), vec![vec![false; 4]; 3], "mas não marca nenhuma célula");
    // digitar numa célula da coluna nova marca só ela
    t.editar_celula(2, 1, "x").unwrap();
    assert_eq!(t.marcas_da_pagina(2, 1), vec![vec![false, true, false, false]]);
    assert_eq!(t.marcas_da_pagina(0, 2), vec![vec![false; 4]; 2]);
    // colar uma coluna: marca o que veio com valor, não o que veio vazio
    t.inserir_coluna_com_valores(4, "cola", &["a".into(), "".into(), "c".into()]).unwrap();
    assert_eq!(
        t.marcas_da_pagina(0, 10),
        vec![
            vec![false, false, false, false, true],
            vec![false, false, false, false, false],
            vec![false, true, false, false, true],
        ]
    );
    // a célula alterada de uma coluna que foi excluída não aparece em lugar nenhum
    t.editar_celula(0, 0, "A").unwrap();
    assert!(t.marcas_da_pagina(0, 1)[0][0]);
    t.excluir_colunas(&[0]).unwrap();
    let marcas = t.marcas_da_pagina(0, 10);
    assert!(marcas.iter().all(|m| m.len() == t.colunas()));
    assert_eq!(marcas[0], vec![false, false, false, true]);
    // a marca acompanha a coluna quando uma inserção antes dela a empurra de lugar
    t.inserir_coluna(0, "antes").unwrap();
    assert_eq!(t.marcas_da_pagina(2, 1), vec![vec![false, true, false, false, true]]);
}

#[test]
fn marcas_da_pagina_acompanha_linha_inserida_e_excluida() {
    let pasta = Pasta::nova("marcas_linha");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    t.editar_celula(2, 0, "ultima").unwrap();
    // linha nova vazia: sem marca, e empurra a marca da linha que estava abaixo
    t.inserir_linha(0).unwrap();
    assert_eq!(t.linhas(), 4);
    assert_eq!(
        t.marcas_da_pagina(0, 10),
        vec![vec![false; 3], vec![false; 3], vec![false; 3], vec![true, false, false]]
    );
    // digitar na linha nova marca só a célula digitada
    t.editar_celula(0, 1, "novo").unwrap();
    assert_eq!(t.marcas_da_pagina(0, 1), vec![vec![false, true, false]]);
    assert_eq!(t.marcas_da_pagina(3, 1), vec![vec![true, false, false]]);
    // excluir uma linha de cima leva a marca dela junto e sobe as de baixo
    t.excluir_linhas(&[1]).unwrap();
    assert_eq!(t.marcas_da_pagina(0, 10), vec![vec![false, true, false], vec![false; 3], vec![true, false, false]]);
    // excluir a linha nova com a célula digitada some com a marca
    t.excluir_linhas(&[0]).unwrap();
    assert_eq!(t.marcas_da_pagina(0, 10), vec![vec![false; 3], vec![true, false, false]]);
    // desfazer traz a linha e a marca de volta
    assert!(t.desfazer());
    assert_eq!(t.marcas_da_pagina(0, 1), vec![vec![false, true, false]]);
    // em toda janela, as marcas têm o formato da página
    for inicio in 0..=t.linhas() + 1 {
        for q in [0usize, 1, 2, 9] {
            let marcas = t.marcas_da_pagina(inicio, q);
            let pagina = t.pagina(inicio, q).unwrap();
            assert_eq!(marcas.len(), pagina.len(), "inicio {inicio}, quantidade {q}");
            assert!(marcas.iter().zip(&pagina).all(|(m, p)| m.len() == p.len()));
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// Teste de modelo: operações aleatórias contra uma tabela de referência
// ---------------------------------------------------------------------------------------------------

struct Aleatorio(u64);

impl Aleatorio {
    fn proximo(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn ate(&mut self, n: usize) -> usize {
        usize::try_from(self.proximo() % n as u64).unwrap()
    }
}

#[derive(Clone, PartialEq, Debug)]
struct Modelo {
    nomes: Vec<String>,
    linhas: Vec<Vec<String>>,
}

const VALORES: [&str; 11] = [
    "",
    "x",
    "com;sep",
    "aspas \"duplas\"",
    "linha 1\r\nlinha 2",
    "só\nLF",
    "ção ç",
    " espaços ",
    "1,5",
    "\"",
    "日本 😀",
];

fn texto_do_modelo(m: &Modelo) -> Vec<u8> {
    let f = Formato::padrao();
    let mut saida = BOM_UTF8.to_vec();
    saida.extend(registro_em_bytes(&m.nomes, &f).unwrap());
    for l in &m.linhas {
        saida.extend(registro_em_bytes(l, &f).unwrap());
    }
    saida
}

fn modelo_inicial(linhas: usize) -> Modelo {
    let nomes = (0..4).map(|j| format!("c{j}")).collect();
    let linhas = (0..linhas)
        .map(|i| {
            vec![
                format!("r{i}"),
                if i % 7 == 0 { "com;sep".into() } else { "texto".into() },
                if i % 11 == 0 {
                    "linha1\r\nlinha2".into()
                } else if i % 13 == 0 {
                    String::new()
                } else {
                    format!("ção {i}")
                },
                if i % 5 == 0 { "diz \"oi\"".into() } else { format!("{i},5") },
            ]
        })
        .collect();
    Modelo { nomes, linhas }
}

fn confere_com_o_modelo(t: &mut Tabela, m: &Modelo, onde: &str) {
    assert_eq!(t.nomes(), m.nomes, "nomes, {onde}");
    assert_eq!(t.linhas(), m.linhas.len() as u64, "linhas, {onde}");
    assert_eq!(tudo(t), m.linhas, "células, {onde}");
    // uma janela no meio, para a leitura por trechos
    if m.linhas.len() > 20 {
        assert_eq!(t.pagina(7, 13).unwrap(), m.linhas[7..20], "janela, {onde}");
    }
}

fn confere_colunas(t: &mut Tabela, m: &Modelo, onde: &str) {
    let todas: Vec<usize> = (0..m.nomes.len()).collect();
    let colunas = t.colunas_de(&todas, &mut sem_progresso).unwrap();
    for (j, coluna) in colunas.iter().enumerate() {
        let esperada: Vec<String> = m.linhas.iter().map(|l| l[j].clone()).collect();
        assert_eq!(*coluna, esperada, "coluna {j}, {onde}");
    }
}

#[test]
fn operacoes_aleatorias_contra_o_modelo() {
    for semente in [1u64, 7, 42, 2026] {
        let pasta = Pasta::nova("modelo");
        let mut modelo = modelo_inicial(150);
        let caminho = pasta.arquivo("m.csv", &texto_do_modelo(&modelo));
        let opcoes = OpcoesAbertura { cabecalho: Some(true), ..OpcoesAbertura::default() };
        let mut t = Tabela::abrir(&caminho, &opcoes).unwrap();
        let mut rng = Aleatorio(semente.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut passados: Vec<Modelo> = Vec::new();
        let mut desfeitos: Vec<Modelo> = Vec::new();
        confere_com_o_modelo(&mut t, &modelo, "inicial");

        for passo in 0..120 {
            let antes = modelo.clone();
            let onde = format!("semente {semente}, passo {passo}");
            let n_linhas = modelo.linhas.len();
            let n_colunas = modelo.nomes.len();
            let mut mudou = true;
            match rng.ate(12) {
                0..=3 if n_linhas > 0 => {
                    let (r, c) = (rng.ate(n_linhas), rng.ate(n_colunas));
                    let v = VALORES[rng.ate(VALORES.len())];
                    let efetivo = modelo.linhas[r][c] != v;
                    let mudou_de_fato = t.editar_celula(r as u64, c, v).unwrap();
                    assert_eq!(mudou_de_fato, efetivo, "{onde}");
                    modelo.linhas[r][c] = v.to_string();
                    mudou = efetivo;
                }
                4 => {
                    let p = rng.ate(n_linhas + 1);
                    t.inserir_linha(p as u64).unwrap();
                    modelo.linhas.insert(p, vec![String::new(); n_colunas]);
                }
                5 if n_linhas > 0 => {
                    let k = 1 + rng.ate(3);
                    let mut alvo: Vec<u64> = (0..k).map(|_| rng.ate(n_linhas) as u64).collect();
                    t.excluir_linhas(&alvo).unwrap();
                    alvo.sort_unstable();
                    alvo.dedup();
                    for &p in alvo.iter().rev() {
                        modelo.linhas.remove(usize::try_from(p).unwrap());
                    }
                }
                6 => {
                    let p = rng.ate(n_colunas + 1);
                    let nome = format!("n{passo}");
                    t.inserir_coluna(p, &nome).unwrap();
                    modelo.nomes.insert(p, nome);
                    for l in &mut modelo.linhas {
                        l.insert(p, String::new());
                    }
                }
                7 if n_colunas > 1 => {
                    let c = rng.ate(n_colunas);
                    t.excluir_colunas(&[c]).unwrap();
                    modelo.nomes.remove(c);
                    for l in &mut modelo.linhas {
                        l.remove(c);
                    }
                }
                8 => {
                    let c = rng.ate(n_colunas);
                    let nome = format!("{}{}", VALORES[rng.ate(VALORES.len())], passo);
                    let efetivo = modelo.nomes[c] != nome;
                    t.renomear_coluna(c, &nome).unwrap();
                    modelo.nomes[c] = nome;
                    mudou = efetivo;
                }
                9 => {
                    // colar uma coluna nova com valores (um passo só)
                    let p = rng.ate(n_colunas + 1);
                    let valores: Vec<String> =
                        (0..n_linhas).map(|_| VALORES[rng.ate(VALORES.len())].to_string()).collect();
                    let nome = format!("p{passo}");
                    t.inserir_coluna_com_valores(p, &nome, &valores).unwrap();
                    modelo.nomes.insert(p, nome);
                    for (l, v) in modelo.linhas.iter_mut().zip(valores) {
                        l.insert(p, v);
                    }
                }
                10 => {
                    if let Some(anterior) = passados.pop() {
                        desfeitos.push(modelo.clone());
                        assert!(t.desfazer(), "{onde}");
                        modelo = anterior;
                    } else {
                        assert!(!t.desfazer());
                    }
                    mudou = false;
                }
                _ => {
                    if let Some(proximo) = desfeitos.pop() {
                        passados.push(modelo.clone());
                        assert!(t.refazer(), "{onde}");
                        modelo = proximo;
                    } else {
                        assert!(!t.refazer());
                    }
                    mudou = false;
                }
            }
            if mudou && modelo != antes {
                passados.push(antes);
                desfeitos.clear();
            }
            confere_com_o_modelo(&mut t, &modelo, &onde);
            if passo % 10 == 0 {
                confere_colunas(&mut t, &modelo, &onde);
            }
            if passo % 40 == 39 {
                // salva no meio: a tabela recomeça do arquivo novo, e o histórico também
                salvar(&mut t);
                assert!(!t.alterada());
                passados.clear();
                desfeitos.clear();
                confere_com_o_modelo(&mut t, &modelo, &format!("{onde}, depois de salvar"));
                confere_o_indice(&t);
            }
        }
        salvar(&mut t);
        confere_com_o_modelo(&mut t, &modelo, "depois do último salvar");
        confere_o_indice(&t);
        // uma leitura nova do arquivo gravado mostra o mesmo
        let mut novo = Tabela::abrir(t.caminho(), &opcoes_de(t.formato())).unwrap();
        confere_com_o_modelo(&mut novo, &modelo, "arquivo reaberto");
        pasta.sem_temporarios();
    }
}

#[test]
fn desfazer_tudo_volta_ao_arquivo_original_byte_a_byte() {
    let pasta = Pasta::nova("desfazer_tudo");
    let modelo = modelo_inicial(100);
    let original = texto_do_modelo(&modelo);
    let mut t = abrir(&pasta, "m.csv", &original);
    t.editar_celula(3, 1, "x").unwrap();
    t.inserir_coluna(1, "novo").unwrap();
    t.excluir_linhas(&[0, 1, 2]).unwrap();
    while t.desfazer() {}
    salvar(&mut t);
    assert_eq!(bytes(&t), original);
    // uma célula que voltou ao valor original regrava a linha igual
    t.editar_celula(0, 0, "tmp").unwrap();
    t.editar_celula(0, 0, "r0").unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), original);
}

#[test]
fn tipos_detectados_das_colunas() {
    use crate::numero::TipoDetectado::{Data, Decimal, Inteiro, Texto, Vazio};
    let pasta = Pasta::nova("tipos");
    let mut t = abrir(
        &pasta,
        "a.csv",
        b"data;regiao;quantidade;valor;vazia;codigo\r\n2025-01-02;Sul;1;12,90;;004718\r\n2025-01-03;Norte;8;178,00;;004725\r\n",
    );
    assert_eq!(t.tipos_das_colunas(100).unwrap(), [Data, Texto, Inteiro, Decimal, Vazio, Texto]);
    // as alterações contam: um texto numa coluna de inteiros a torna texto
    t.editar_celula(0, 2, "muitas").unwrap();
    assert_eq!(t.tipos_das_colunas(100).unwrap()[2], Texto);
    // e a amostra limita
    t.desfazer();
    t.inserir_linha(2).unwrap();
    t.editar_celula(2, 2, "muitas").unwrap();
    assert_eq!(t.tipos_das_colunas(2).unwrap()[2], Inteiro);
    assert_eq!(t.tipos_das_colunas(3).unwrap()[2], Texto);
}

#[test]
fn celula_maior_que_o_buffer() {
    let pasta = Pasta::nova("gigante");
    let gigante = "x".repeat(3 << 20);
    let texto = format!("id;obs\r\n1;ok\r\n2;\"{gigante}\r\nfim\"\r\n3;ok\r\n");
    let mut t = abrir(&pasta, "g.csv", texto.as_bytes());
    assert_eq!(t.linhas(), 3);
    assert_eq!(t.celula(1, 1).unwrap(), format!("{gigante}\r\nfim"));
    assert_eq!(t.celula(2, 1).unwrap(), "ok");
    t.editar_celula(2, 0, "9").unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), texto.replace("3;ok", "9;ok").as_bytes(), "a célula gigante foi copiada como estava");
    confere_o_indice(&t);
}

#[test]
fn pagina_com_quantidade_enorme_nao_estoura() {
    let pasta = Pasta::nova("quantidade_enorme");
    let mut t = abrir(&pasta, "a.csv", EXPORT.as_bytes());
    assert_eq!(t.pagina(0, usize::MAX).unwrap().len(), 3);
    assert_eq!(t.pagina(1, usize::MAX).unwrap().len(), 2);
    let mut leitor = Leitor::abrir(t.caminho(), &OpcoesAbertura::default()).unwrap();
    assert_eq!(leitor.linhas_de(0, usize::MAX).unwrap().len(), 3);
}

#[test]
fn salvar_arquivo_vazio_depois_de_inserir() {
    let pasta = Pasta::nova("vazio_inserir");
    let mut t = abrir(&pasta, "a.csv", b"");
    t.inserir_linha(0).unwrap();
    t.editar_celula(0, 0, "ola").unwrap();
    t.inserir_linha(1).unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), b"ola\r\n\"\"\r\n", "a linha vazia de uma coluna só sai como \"\"");
    assert_eq!(tudo(&mut t), l(&[&["ola"], &[""]]));
    confere_o_indice(&t);
    // com cabeçalho forçado num arquivo vazio, o cabeçalho sai dos nomes
    let caminho = pasta.arquivo("b.csv", b"");
    let opcoes = OpcoesAbertura { cabecalho: Some(true), ..OpcoesAbertura::default() };
    let mut t = Tabela::abrir(&caminho, &opcoes).unwrap();
    t.renomear_coluna(0, "nome").unwrap();
    t.inserir_linha(0).unwrap();
    t.editar_celula(0, 0, "Ana").unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), b"nome\r\nAna\r\n");
    confere_o_indice(&t);
}

#[test]
fn linha_so_com_bom_ou_vazia_no_meio_vira_linha_vazia_e_volta_igual() {
    let pasta = Pasta::nova("em_branco");
    let original = b"a;b\r\n1;2\r\n\r\n3;4\r\n".to_vec();
    let mut t = abrir(&pasta, "a.csv", &original);
    t.editar_celula(0, 0, "9").unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), b"a;b\r\n9;2\r\n\r\n3;4\r\n", "a linha em branco não foi tocada");
    t.editar_celula(1, 1, "x").unwrap();
    salvar(&mut t);
    assert_eq!(bytes(&t), b"a;b\r\n9;2\r\n;x\r\n3;4\r\n", "editada, vira uma linha de duas células");
}

// ---------------------------------------------------------------------------------------------------
// Benchmark
// ---------------------------------------------------------------------------------------------------

fn gerar(caminho: &Path, linhas: u64, colunas: usize) {
    let mut w = std::io::BufWriter::with_capacity(1 << 20, fs::File::create(caminho).unwrap());
    let nomes: Vec<String> = (0..colunas).map(|j| format!("coluna{j}")).collect();
    w.write_all(&BOM_UTF8).unwrap();
    w.write_all(nomes.join(";").as_bytes()).unwrap();
    w.write_all(b"\r\n").unwrap();
    let mut linha = String::new();
    for i in 0..linhas {
        linha.clear();
        for j in 0..colunas {
            if j > 0 {
                linha.push(';');
            }
            match j % 5 {
                0 => linha.push_str(&i.to_string()),
                1 => linha.push_str(&format!("produto {}", i % 9973)),
                2 => linha.push_str(&format!("{},{:02}", i % 1000, i % 100)),
                3 => linha.push_str(&format!("\"texto; com separador {}\"", i % 17)),
                _ => linha.push_str("2026-10-17"),
            }
        }
        linha.push_str("\r\n");
        w.write_all(linha.as_bytes()).unwrap();
    }
    w.flush().unwrap();
}

#[test]
#[ignore = "benchmark: cargo test --release -p tabular -- --ignored --nocapture"]
fn indice_de_1_milhao_de_linhas_por_10_colunas() {
    let pasta = Pasta::nova("milhao");
    let caminho = pasta.join("milhao.csv");
    let t0 = Instant::now();
    gerar(&caminho, 1_000_000, 10);
    let bytes_do_arquivo = fs::metadata(&caminho).unwrap().len();
    eprintln!("gerado: {} MB em {:?}", bytes_do_arquivo >> 20, t0.elapsed());

    let t0 = Instant::now();
    let mut tabela = Tabela::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
    let abrir_e_indexar = t0.elapsed();
    eprintln!("abrir e indexar 1.000.000 x 10: {abrir_e_indexar:?}");
    assert_eq!((tabela.linhas(), tabela.colunas()), (1_000_000, 10));
    assert_eq!(tabela.formato().separador, ';');

    let t0 = Instant::now();
    let pagina = tabela.pagina(999_900, 100).unwrap();
    eprintln!("uma página de 100 linhas no fim: {:?}", t0.elapsed());
    assert_eq!(pagina.len(), 100);
    assert_eq!(pagina[99][0], "999999");

    let t0 = Instant::now();
    for k in 0..200u64 {
        tabela.pagina((k * 4999) % 999_000, 50).unwrap();
    }
    eprintln!("200 páginas de 50 linhas em posições espalhadas: {:?}", t0.elapsed());

    let t0 = Instant::now();
    let coluna = tabela.colunas_de(&[0], &mut sem_progresso).unwrap();
    eprintln!("ler a coluna 0 inteira: {:?}", t0.elapsed());
    assert_eq!(coluna[0].len(), 1_000_000);

    let limite = if cfg!(debug_assertions) { 120 } else { 5 };
    assert!(abrir_e_indexar.as_secs() < limite, "indexar demorou demais: {abrir_e_indexar:?}");

    // salvar uma célula: regrava uma linha, copia o resto
    tabela.editar_celula(500_000, 1, "alterado").unwrap();
    let t0 = Instant::now();
    let r = tabela.salvar(&mut sem_progresso).unwrap();
    eprintln!("salvar com uma célula alterada (copia {} MB): {:?}", bytes_do_arquivo >> 20, t0.elapsed());
    assert_eq!(r.linhas_regravadas, 1);
    assert_eq!(tabela.celula(500_000, 1).unwrap(), "alterado");
    assert_eq!(tabela.linhas(), 1_000_000);
}
