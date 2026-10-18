use std::fs;
use std::time::{Duration, Instant};

use super::*;
use crate::testes_apoio::Pasta;

fn carregar_bytes(pasta: &Pasta, bytes: &[u8]) -> Carregado {
    carregar(&pasta.arquivo("a.txt", bytes)).unwrap()
}

#[test]
fn carrega_utf8_com_bom_e_windows_1252() {
    let pasta = Pasta::nova("codificacoes");
    let c = carregar_bytes(&pasta, "ação\n€".as_bytes());
    assert_eq!((c.codificacao, c.texto.as_str()), (Codificacao::Utf8, "ação\n€"));

    let mut com_bom = BOM_UTF8.to_vec();
    com_bom.extend_from_slice("ação\n".as_bytes());
    let c = carregar_bytes(&pasta, &com_bom);
    assert_eq!((c.codificacao, c.texto.as_str()), (Codificacao::Utf8ComBom, "ação\n"), "o BOM não entra no texto");
    assert_eq!(c.bytes, com_bom.len() as u64, "o tamanho é o do arquivo, com o BOM");

    let c = carregar_bytes(&pasta, b"a\xE7\xE3o \x80 \x93x\x94");
    assert_eq!((c.codificacao, c.texto.as_str()), (Codificacao::Windows1252, "ação € “x”"));

    let c = carregar_bytes(&pasta, b"so ascii");
    assert_eq!(c.codificacao, Codificacao::Utf8, "só ASCII é UTF-8");
    assert_eq!(c.rotulo_da_codificacao(), "UTF-8");

    // UTF-8 válido no começo e byte solto lá no fim: o arquivo inteiro é conferido
    let mut longo = "a".repeat(100_000).into_bytes();
    longo.extend_from_slice(b"\xE7");
    assert_eq!(carregar_bytes(&pasta, &longo).codificacao, Codificacao::Windows1252);

    // BOM de UTF-8 e depois bytes que não são UTF-8: lido como Windows-1252, sem perder nada
    let c = carregar_bytes(&pasta, b"\xEF\xBB\xBFa\xE7");
    assert_eq!(c.codificacao, Codificacao::Windows1252);
    assert_eq!(codificar(&c.texto, c.codificacao, c.fim_de_linha).unwrap(), b"\xEF\xBB\xBFa\xE7");
}

#[test]
fn arquivo_vazio_e_sem_quebra() {
    let pasta = Pasta::nova("vazio");
    let c = carregar_bytes(&pasta, b"");
    assert_eq!(c.texto, "");
    assert_eq!(
        (c.codificacao, c.fim_de_linha, c.fim_misto, c.termina_em_quebra),
        (Codificacao::Utf8, FimDeLinha::Crlf, false, false)
    );
    let c = carregar_bytes(&pasta, b"uma linha");
    assert_eq!((c.fim_de_linha, c.termina_em_quebra), (FimDeLinha::Crlf, false), "sem quebra: o padrão do Windows");
}

#[test]
fn utf16_e_bytes_nulos_dao_a_frase() {
    let pasta = Pasta::nova("utf16");
    let utf16 =
        "O arquivo está em UTF-16, que o DataBox ainda não abre como texto. Salve-o como UTF-8 no programa de origem.";
    // com BOM, nas duas ordens
    for bom in [[0xFF, 0xFE], [0xFE, 0xFF]] {
        let mut b = bom.to_vec();
        b.extend_from_slice(b"a\0b\0");
        let e = carregar(&pasta.arquivo("u.txt", &b)).unwrap_err();
        assert_eq!((e.mensagem.as_str(), e.status), (utf16, 400));
    }
    // sem BOM: o compasso de zeros de dois em dois
    let sem_bom: Vec<u8> = "texto em utf-16".encode_utf16().flat_map(u16::to_le_bytes).collect();
    assert_eq!(carregar(&pasta.arquivo("u2.txt", &sem_bom)).unwrap_err().mensagem, utf16);
    let sem_bom: Vec<u8> = "texto em utf-16".encode_utf16().flat_map(u16::to_be_bytes).collect();
    assert_eq!(carregar(&pasta.arquivo("u3.txt", &sem_bom)).unwrap_err().mensagem, utf16);
    // bytes nulos soltos num arquivo que não é UTF-16: não é texto
    let mut binario = b"MZ\x90 cabecalho de programa ".to_vec();
    binario.extend_from_slice(&[0, 0, 0, 0]);
    binario.extend_from_slice(b" e mais coisa que nao tem nada de utf-16 aqui dentro, bem longa");
    let e = carregar(&pasta.arquivo("p.exe", &binario)).unwrap_err();
    assert_eq!(e.mensagem, "O arquivo não parece de texto (tem bytes nulos); o DataBox só abre arquivos de texto.");
    // um só byte nulo no fim de tudo também recusa
    assert!(carregar(&pasta.arquivo("z.txt", b"abc\0")).is_err());
    assert!(carregar(&pasta.arquivo("z2.txt", b"\0")).is_err());
}

#[test]
fn fim_de_linha_dominante_e_misto() {
    let pasta = Pasta::nova("fim_de_linha");
    let c = carregar_bytes(&pasta, b"a\r\nb\r\nc\r\n");
    assert_eq!((c.fim_de_linha, c.fim_misto, c.texto.as_str()), (FimDeLinha::Crlf, false, "a\nb\nc\n"));
    assert_eq!(c.rotulo_do_fim_de_linha(), "CRLF");

    let c = carregar_bytes(&pasta, b"a\nb\nc");
    assert_eq!((c.fim_de_linha, c.fim_misto, c.texto.as_str()), (FimDeLinha::Lf, false, "a\nb\nc"));
    assert_eq!(c.rotulo_do_fim_de_linha(), "LF");

    let c = carregar_bytes(&pasta, b"a\rb\rc\r");
    assert_eq!((c.fim_de_linha, c.fim_misto, c.texto.as_str()), (FimDeLinha::Cr, false, "a\nb\nc\n"));
    assert_eq!(c.rotulo_do_fim_de_linha(), "CR");

    // mistura: vence o que mais aparece, e a ficha diz "(misto)"
    let c = carregar_bytes(&pasta, b"a\r\nb\r\nc\nd\r\ne\n");
    assert_eq!((c.fim_de_linha, c.fim_misto, c.texto.as_str()), (FimDeLinha::Crlf, true, "a\nb\nc\nd\ne\n"));
    assert_eq!(c.rotulo_do_fim_de_linha(), "CRLF (misto)");
    let c = carregar_bytes(&pasta, b"a\nb\nc\nd\r\ne\rf");
    assert_eq!((c.fim_de_linha, c.fim_misto, c.texto.as_str()), (FimDeLinha::Lf, true, "a\nb\nc\nd\ne\nf"));
    assert_eq!(c.rotulo_do_fim_de_linha(), "LF (misto)");
    // empate: CRLF vence LF, e LF vence CR
    let c = carregar_bytes(&pasta, b"a\r\nb\nc");
    assert_eq!((c.fim_de_linha, c.fim_misto), (FimDeLinha::Crlf, true));
    let c = carregar_bytes(&pasta, b"a\nb\rc");
    assert_eq!((c.fim_de_linha, c.fim_misto), (FimDeLinha::Lf, true));
    let c = carregar_bytes(&pasta, b"a\r\nb\rc");
    assert_eq!((c.fim_de_linha, c.fim_misto), (FimDeLinha::Crlf, true));
    // \r\r\n são um CR e um CRLF; \n\r é um LF e um CR
    let c = carregar_bytes(&pasta, b"a\r\r\nb");
    assert_eq!(c.texto, "a\n\nb");
    let c = carregar_bytes(&pasta, b"a\n\rb");
    assert_eq!(c.texto, "a\n\nb");
    // o texto nunca guarda \r
    assert!(!carregar_bytes(&pasta, b"x\r\ny\rz\nw\r\n").texto.contains('\r'));
}

#[test]
fn grava_de_volta_com_crlf_e_bom() {
    let pasta = Pasta::nova("volta");
    let mut origem = BOM_UTF8.to_vec();
    origem.extend_from_slice("linha ç\r\nsegunda €\r\n".as_bytes());
    let caminho = pasta.arquivo("a.txt", &origem);
    let c = carregar(&caminho).unwrap();
    assert_eq!((c.codificacao, c.fim_de_linha), (Codificacao::Utf8ComBom, FimDeLinha::Crlf));

    // sem mexer: byte a byte igual
    let resumo = gravar(&caminho, &c.texto, &c).unwrap();
    assert_eq!(fs::read(&caminho).unwrap(), origem);
    assert_eq!(resumo.bytes, origem.len() as u64);
    assert!(resumo.modificado.is_some());
    pasta.sem_temporarios();

    // com edição: o BOM e o CRLF voltam, inclusive nas linhas novas
    let editado = format!("{}terceira\nquarta", c.texto);
    gravar(&caminho, &editado, &c).unwrap();
    let mut esperado = BOM_UTF8.to_vec();
    esperado.extend_from_slice("linha ç\r\nsegunda €\r\nterceira\r\nquarta".as_bytes());
    assert_eq!(fs::read(&caminho).unwrap(), esperado);

    // LF e CR também
    for (fim, bytes) in [(FimDeLinha::Lf, "a\nb\n"), (FimDeLinha::Cr, "a\rb\r")] {
        let caminho = pasta.arquivo("b.txt", bytes.as_bytes());
        let c = carregar(&caminho).unwrap();
        assert_eq!(c.fim_de_linha, fim);
        gravar(&caminho, &format!("{}c\n", c.texto), &c).unwrap();
        let esperado = format!("{bytes}c{}", if fim == FimDeLinha::Lf { "\n" } else { "\r" });
        assert_eq!(fs::read(&caminho).unwrap(), esperado.as_bytes());
    }

    // arquivo misto: regravado todo com o dominante
    let caminho = pasta.arquivo("m.txt", b"a\r\nb\r\nc\nd");
    let c = carregar(&caminho).unwrap();
    assert!(c.fim_misto);
    gravar(&caminho, &c.texto, &c).unwrap();
    assert_eq!(fs::read(&caminho).unwrap(), b"a\r\nb\r\nc\r\nd");
}

#[test]
fn ida_e_volta_de_todas_as_combinacoes_sem_mudar_um_byte() {
    let pasta = Pasta::nova("ida_e_volta");
    let corpos: [&[u8]; 4] = [
        b"",
        b"sem quebra nenhuma",
        "acentua\u{e7}\u{e3}o e \u{20ac} e \u{1f600}\nsegunda\n".as_bytes(),
        b"\n\n\nlinhas vazias\n\n",
    ];
    for corpo in corpos {
        for bom in [false, true] {
            for quebra in [&b"\r\n"[..], b"\n", b"\r"] {
                let mut bytes = if bom { BOM_UTF8.to_vec() } else { Vec::new() };
                let texto = String::from_utf8_lossy(corpo).replace('\n', &String::from_utf8_lossy(quebra));
                bytes.extend_from_slice(texto.as_bytes());
                let caminho = pasta.arquivo("x.txt", &bytes);
                let c = carregar(&caminho).unwrap();
                gravar(&caminho, &c.texto, &c).unwrap();
                assert_eq!(fs::read(&caminho).unwrap(), bytes, "bom={bom} quebra={quebra:?} corpo={corpo:?}");
            }
        }
    }
    // Windows-1252 com todos os bytes que ele tem (menos os da quebra e o nulo)
    let todos: Vec<u8> = (1..=255u8).filter(|&b| b != b'\n' && b != b'\r').collect();
    let caminho = pasta.arquivo("w.txt", &todos);
    let c = carregar(&caminho).unwrap();
    assert_eq!(c.codificacao, Codificacao::Windows1252);
    gravar(&caminho, &c.texto, &c).unwrap();
    assert_eq!(fs::read(&caminho).unwrap(), todos);
}

#[test]
fn grava_sem_a_quebra_final_se_nao_tinha() {
    let pasta = Pasta::nova("quebra_final");
    // não tinha: continua sem
    let caminho = pasta.arquivo("a.txt", b"a\r\nb");
    let c = carregar(&caminho).unwrap();
    assert!(!c.termina_em_quebra);
    gravar(&caminho, &c.texto, &c).unwrap();
    assert_eq!(fs::read(&caminho).unwrap(), b"a\r\nb");

    // tinha: continua com
    let caminho = pasta.arquivo("b.txt", b"a\r\nb\r\n");
    let mut c = carregar(&caminho).unwrap();
    assert!(c.termina_em_quebra);
    gravar(&caminho, &c.texto, &c).unwrap();
    assert_eq!(fs::read(&caminho).unwrap(), b"a\r\nb\r\n");

    // a pessoa decide: apagar a quebra do fim grava sem ela, e acrescentar grava com ela
    let sem = c.texto.trim_end_matches('\n').to_string();
    let r = gravar(&caminho, &sem, &c).unwrap();
    assert_eq!(fs::read(&caminho).unwrap(), b"a\r\nb");
    c.registrar_gravacao(&r, &sem);
    assert!(!c.termina_em_quebra && !c.fim_misto);
    assert_eq!(c.bytes, 4);
    assert!(!mudou_no_disco(&caminho, &c), "depois de registrar, o arquivo novo é o conhecido");
    gravar(&caminho, "a\nb\n", &c).unwrap();
    assert_eq!(fs::read(&caminho).unwrap(), b"a\r\nb\r\n");

    // texto vazio grava um arquivo vazio (sem BOM de brinde quando não tinha)
    gravar(&caminho, "", &c).unwrap();
    assert_eq!(fs::read(&caminho).unwrap(), b"");
}

#[test]
fn windows_1252_recusa_o_caractere_que_nao_tem() {
    let pasta = Pasta::nova("recusa");
    let caminho = pasta.arquivo("a.txt", b"a\xE7\xE3o\nsegunda\n");
    let c = carregar(&caminho).unwrap();
    assert_eq!(c.codificacao, Codificacao::Windows1252);
    let antes = fs::read(&caminho).unwrap();

    let e = gravar(&caminho, "ação\nsegunda\nterceira 日本\n", &c).unwrap_err();
    assert_eq!(
        e.mensagem,
        "O caractere '日' (linha 3) não existe na codificação Windows-1252, que é a deste arquivo. Troque-o para poder \
         salvar."
    );
    assert_eq!(e.status, 400);
    assert_eq!(fs::read(&caminho).unwrap(), antes, "nada foi gravado");
    pasta.sem_temporarios();

    // o primeiro da lista, não o último; e na linha 1
    let e = gravar(&caminho, "\u{1f600} e 日", &c).unwrap_err();
    assert!(e.mensagem.starts_with("O caractere '\u{1f600}' (linha 1)"), "{}", e.mensagem);
    // os que o Windows-1252 tem passam
    gravar(&caminho, "€ “aspas” — ÿ\n", &c).unwrap();
    assert_eq!(fs::read(&caminho).unwrap(), b"\x80 \x93aspas\x94 \x97 \xFF\n", "o fim de linha é o do arquivo (LF)");
    // UTF-8 aceita tudo
    let utf8 = carregar(&pasta.arquivo("u.txt", b"x")).unwrap();
    gravar(&pasta.join("u.txt"), "日本\u{1f600}", &utf8).unwrap();
    assert_eq!(fs::read(pasta.join("u.txt")).unwrap(), "日本\u{1f600}".as_bytes());
}

#[test]
fn grava_atomico_apaga_o_temporario_no_erro() {
    let pasta = Pasta::nova("atomico");
    let destino = pasta.arquivo("dados.txt", b"velho");

    // a troca falha: o temporário some e o destino fica como estava
    let r = gravar_atomico_com(&destino, b"novo", |_, _| Err(io::Error::from(io::ErrorKind::PermissionDenied)));
    let e = r.unwrap_err();
    assert_eq!(e.status, 403);
    assert_eq!(fs::read(&destino).unwrap(), b"velho");
    pasta.sem_temporarios();
    assert_eq!(pasta.nomes(), ["dados.txt"]);

    // a troca que dá certo: o destino é o novo e o temporário não sobra
    gravar_atomico(&destino, b"novo").unwrap();
    assert_eq!(fs::read(&destino).unwrap(), b"novo");
    pasta.sem_temporarios();

    // o temporário fica na pasta do destino, com o nome dele
    let mut visto = None;
    gravar_atomico_com(&destino, b"x", |tmp, alvo| {
        visto = Some(tmp.to_path_buf());
        fs::rename(tmp, alvo)
    })
    .unwrap();
    let tmp = visto.unwrap();
    assert_eq!(tmp.parent(), destino.parent());
    let nome = tmp.file_name().unwrap().to_string_lossy().into_owned();
    assert!(nome.starts_with("~dados.txt.") && nome.ends_with(".tmp"), "{nome}");

    // arquivo novo (sem destino anterior)
    let novo = pasta.join("novo.txt");
    gravar_atomico(&novo, b"nascido").unwrap();
    assert_eq!(fs::read(&novo).unwrap(), b"nascido");
}

#[test]
fn destino_com_problema_recusa_com_a_frase() {
    let pasta = Pasta::nova("destino");
    let e = gravar_atomico(&pasta.join("nao_existe").join("x.txt"), b"x").unwrap_err();
    assert_eq!(e.status, 404);
    assert!(e.mensagem.starts_with("A pasta de destino não existe"), "{}", e.mensagem);

    let sub = pasta.join("sub");
    fs::create_dir(&sub).unwrap();
    assert!(gravar_atomico(&sub, b"x").unwrap_err().mensagem.contains("é uma pasta"));

    let ro = pasta.arquivo("ro.txt", b"x");
    let mut p = fs::metadata(&ro).unwrap().permissions();
    p.set_readonly(true);
    fs::set_permissions(&ro, p).unwrap();
    let e = gravar_atomico(&ro, b"y").unwrap_err();
    assert_eq!(e.status, 403);
    assert!(e.mensagem.contains("somente leitura"));
    let mut p = fs::metadata(&ro).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    p.set_readonly(false);
    fs::set_permissions(&ro, p).unwrap();
    assert_eq!(fs::read(&ro).unwrap(), b"x");
    pasta.sem_temporarios();
}

/// O arquivo aberto por outro programa sem deixar trocar: a troca insiste uns 200 ms, desiste com a frase de
/// permissão e não deixa temporário.
#[cfg(windows)]
#[test]
fn arquivo_preso_por_outro_programa_desiste_depois_de_insistir() {
    use std::os::windows::fs::OpenOptionsExt;
    let pasta = Pasta::nova("preso");
    let destino = pasta.arquivo("preso.txt", b"velho");
    let trava = OpenOptions::new().read(true).share_mode(0).open(&destino).unwrap();
    let inicio = Instant::now();
    let e = gravar_atomico(&destino, b"novo").unwrap_err();
    assert!(inicio.elapsed() >= Duration::from_millis(150), "insistiu só {:?}", inicio.elapsed());
    assert_eq!(e.status, 403);
    assert!(e.mensagem.contains("aberto em outro programa"), "{}", e.mensagem);
    drop(trava);
    assert_eq!(fs::read(&destino).unwrap(), b"velho");
    pasta.sem_temporarios();
    // solto, grava
    gravar_atomico(&destino, b"novo").unwrap();
    assert_eq!(fs::read(&destino).unwrap(), b"novo");
}

#[test]
fn acima_do_limite_recusa() {
    let pasta = Pasta::nova("limite");
    // um byte acima: recusa pelo tamanho, sem ler (o conteúdo nem importa)
    let grande = pasta.join("grande.txt");
    let f = File::create(&grande).unwrap();
    f.set_len(LIMITE_LEITURA + 1).unwrap();
    drop(f);
    let e = carregar(&grande).unwrap_err();
    assert_eq!(e.status, 400);
    assert_eq!(e.mensagem, "Este arquivo é grande demais para o DataBox mostrar (32 MB). Abra no programa padrão.");

    let f = File::create(&grande).unwrap();
    f.set_len(LIMITE_LEITURA + 5_000_000).unwrap();
    drop(f);
    assert_eq!(
        carregar(&grande).unwrap_err().mensagem,
        "Este arquivo é grande demais para o DataBox mostrar (36,8 MB). Abra no programa padrão."
    );

    // no limite exato, abre (32 MiB de linhas de 63 letras e a quebra)
    let linha = format!("{}\n", "a".repeat(63));
    let corpo = linha.repeat(usize::try_from(LIMITE_LEITURA).unwrap() / linha.len());
    assert_eq!(corpo.len() as u64, LIMITE_LEITURA);
    let caminho = pasta.arquivo("no_limite.txt", corpo.as_bytes());
    let inicio = Instant::now();
    let c = carregar(&caminho).unwrap();
    eprintln!("32 MiB carregados em {:?}", inicio.elapsed());
    assert_eq!(c.texto.len() as u64, LIMITE_LEITURA);
    assert_eq!((c.codificacao, c.fim_de_linha), (Codificacao::Utf8, FimDeLinha::Lf));
}

#[test]
fn arquivo_inexistente_pasta_e_sem_permissao_tem_frase() {
    let pasta = Pasta::nova("erros");
    let e = carregar(&pasta.join("nao_existe.txt")).unwrap_err();
    assert_eq!(e.status, 404);
    assert!(
        e.mensagem.starts_with("Não foi possível ler o arquivo. Arquivo ou pasta não encontrado:"),
        "{}",
        e.mensagem
    );
    let e = carregar(pasta.caminho()).unwrap_err();
    // no Windows abrir uma pasta falha com acesso negado; em outros sistemas é "não é um arquivo"
    assert!(e.status == 403 || e.mensagem.starts_with("Isto não é um arquivo"), "{e}");
}

#[test]
fn mudou_no_disco_pelo_tamanho_e_pela_data() {
    let pasta = Pasta::nova("mudou");
    let caminho = pasta.arquivo("a.txt", b"conteudo");
    let c = carregar(&caminho).unwrap();
    assert!(!mudou_no_disco(&caminho, &c), "intacto");

    let antes = fs::metadata(&caminho).unwrap().modified().unwrap();

    // outro tamanho
    fs::write(&caminho, b"conteudo maior").unwrap();
    assert!(mudou_no_disco(&caminho, &c), "tamanho");

    // mesmo tamanho, outra data
    fs::write(&caminho, b"CONTEUDO").unwrap();
    let f = OpenOptions::new().write(true).open(&caminho).unwrap();
    f.set_modified(antes + Duration::from_secs(5)).unwrap();
    drop(f);
    assert!(mudou_no_disco(&caminho, &c), "data");

    // a data de volta ao que era: igual
    let f = OpenOptions::new().write(true).open(&caminho).unwrap();
    f.set_modified(antes).unwrap();
    drop(f);
    assert!(!mudou_no_disco(&caminho, &c), "só o conteúdo trocado, com tamanho e data iguais, não aparece");

    // sem data conhecida na leitura: só vale o tamanho
    let sem_data = Carregado { modificado: None, ..c.clone() };
    assert!(!mudou_no_disco(&caminho, &sem_data));

    // sumiu
    fs::remove_file(&caminho).unwrap();
    assert!(mudou_no_disco(&caminho, &c), "sumiu");

    // o que o Salvar do próprio editor faz não conta como mudança
    let caminho = pasta.arquivo("b.txt", b"um");
    let mut c = carregar(&caminho).unwrap();
    let r = gravar(&caminho, "dois dois", &c).unwrap();
    assert!(mudou_no_disco(&caminho, &c));
    c.registrar_gravacao(&r, "dois dois");
    assert!(!mudou_no_disco(&caminho, &c));
}

#[test]
fn decodificar_serve_a_quem_ja_tem_os_bytes() {
    let c = decodificar(b"a\r\nb", None).unwrap();
    assert_eq!((c.texto.as_str(), c.bytes, c.modificado), ("a\nb", 4, None));
    assert!(decodificar(&[0xFF, 0xFE, 0x61, 0x00], None).is_err());
}

#[test]
fn codificar_so_troca_a_quebra_e_poe_o_bom() {
    let bytes = codificar("a\nb\n\nç", Codificacao::Utf8ComBom, FimDeLinha::Crlf).unwrap();
    assert_eq!(bytes, b"\xEF\xBB\xBFa\r\nb\r\n\r\n\xC3\xA7");
    assert_eq!(codificar("a\nb", Codificacao::Windows1252, FimDeLinha::Cr).unwrap(), b"a\rb");
    assert_eq!(codificar("", Codificacao::Utf8ComBom, FimDeLinha::Lf).unwrap(), BOM_UTF8);
    assert_eq!(codificar("", Codificacao::Utf8, FimDeLinha::Lf).unwrap(), b"");
}

#[cfg(windows)]
#[test]
fn gravar_por_um_link_simbolico_grava_o_arquivo_de_verdade() {
    let pasta = Pasta::nova("link_simbolico");
    let alvo = pasta.arquivo("alvo.txt", b"antes\n");
    let link = alvo.with_file_name("atalho.txt");
    // criar o link pede o modo de desenvolvedor (ou admin) no Windows: sem ele, não há o que conferir
    if std::os::windows::fs::symlink_file(&alvo, &link).is_err() {
        return;
    }
    gravar_atomico(&link, b"depois\n").unwrap();
    assert_eq!(fs::read(&alvo).unwrap(), b"depois\n", "o arquivo para onde o link aponta mudou");
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "o link continua link");
}
