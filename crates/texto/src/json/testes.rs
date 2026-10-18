use std::thread;
use std::time::Instant;

use super::*;
use crate::posicao::{byte_da_linha_e_coluna, linha_e_coluna};

/// O documento da prancheta 8 (24 linhas, 2 espaços, com a quebra no fim).
const EXEMPLO: &str = r#"{
  "versao": 3,
  "tema": "escuro",
  "idioma": "pt-BR",
  "conexoes": [
    {
      "nome": "Produção",
      "arquivo": "C:\\dados\\exemplo.db",
      "tempo_limite": 30,
      "somente_leitura": false
    },
    {
      "nome": "Relatórios",
      "arquivo": "D:\\relatorios\\financeiro.sqlite",
      "tempo_limite": 60,
      "somente_leitura": true
    }
  ],
  "exportacao": {
    "formato": "xlsx",
    "separador": ";",
    "cabecalho": true
  }
}
"#;

/// Um documento com todos os tipos, compacto, para os testes de propriedade.
const COMPACTO: &str = r#"{"n":[1,-2,3.5,1e5,-0,0.25e-3],"s":"a\"b\\c\/d\u00e7\ud83d\ude00\n","t":true,"f":false,"z":null,"o":{"a":{"b":[[],{}]}},"v":[],"w":{},"çã":"ação €"}"#;

fn erro(texto: &str) -> ErroJson {
    analisar(texto).erro.unwrap_or_else(|| panic!("devia dar erro: {texto:?}"))
}

fn conferir(texto: &str, linha: usize, coluna: usize, motivo: &str) {
    let e = erro(texto);
    assert_eq!((e.linha, e.coluna, e.motivo.as_str()), (linha, coluna, motivo), "{texto:?}");
    // o byte e a linha e coluna dizem a mesma coisa
    assert_eq!(linha_e_coluna(texto, e.byte), (linha, coluna), "{texto:?}");
}

fn valido(texto: &str) -> Arvore {
    let a = analisar(texto);
    assert!(a.erro.is_none(), "devia ser válido: {texto:?}: {:?}", a.erro);
    a.arvore.unwrap_or_else(|| panic!("válido sem árvore: {texto:?}"))
}

/// O documento sem os espaços e quebras que ficam fora das cadeias.
fn compacto(texto: &str) -> String {
    let (mut saida, mut dentro, mut escape) = (String::new(), false, false);
    for c in texto.chars() {
        if dentro {
            saida.push(c);
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                dentro = false;
            }
        } else if c == '"' {
            dentro = true;
            saida.push(c);
        } else if !matches!(c, ' ' | '\t' | '\n' | '\r') {
            saida.push(c);
        }
    }
    saida
}

fn nomes_dos_filhos(a: &Arvore, id: NoId, texto: &str) -> Vec<String> {
    a.filhos(id).map(|f| a.nome(f, texto)).collect()
}

fn tipos_dos_filhos(a: &Arvore, id: NoId) -> Vec<Tipo> {
    a.filhos(id).map(|f| a.no(f).tipo).collect()
}

fn filho(a: &Arvore, id: NoId, n: usize) -> NoId {
    a.filhos(id).nth(n).unwrap_or_else(|| panic!("sem o filho {n}"))
}

/// As invariantes de uma árvore montada: cada nó dentro do pai, as chaves antes dos valores, os começos em ordem, a
/// contagem de filhos igual ao que a iteração dá e o texto de cada valor com o formato do tipo.
fn conferir_arvore(a: &Arvore, texto: &str) {
    let mut ultimo_comeco = 0;
    for i in 0..a.total() {
        let id = NoId(u32::try_from(i).unwrap());
        let no = a.no(id);
        assert!(no.inicio < no.fim && no.fim <= texto.len(), "nó {i}: {}..{}", no.inicio, no.fim);
        let valor = &texto[no.inicio..no.fim];
        match no.tipo {
            Tipo::Objeto => assert!(valor.starts_with('{') && valor.ends_with('}'), "{valor:?}"),
            Tipo::Lista => assert!(valor.starts_with('[') && valor.ends_with(']'), "{valor:?}"),
            Tipo::Texto => assert!(valor.starts_with('"') && valor.ends_with('"') && valor.len() >= 2, "{valor:?}"),
            Tipo::Booleano => assert!(valor == "true" || valor == "false", "{valor:?}"),
            Tipo::Nulo => assert_eq!(valor, "null"),
            Tipo::Inteiro => assert!(valor.trim_start_matches('-').bytes().all(|b| b.is_ascii_digit()), "{valor:?}"),
            Tipo::Numero => assert!(valor.contains(['.', 'e', 'E']), "{valor:?}"),
        }
        let comeco = no.chave.as_ref().map_or(no.inicio, |c| c.start);
        assert!(comeco >= ultimo_comeco, "começos fora de ordem no nó {i}");
        ultimo_comeco = comeco;
        if let Some(chave) = &no.chave {
            assert!(chave.end <= no.inicio, "a chave vem antes do valor");
            assert!(texto[chave.clone()].starts_with('"') && texto[chave.clone()].ends_with('"'));
        }
        let filhos: Vec<NoId> = a.filhos(id).collect();
        assert_eq!(filhos.len(), no.filhos as usize, "contagem de filhos do nó {i}");
        assert!(no.tipo.e_container() || filhos.is_empty());
        let mut fim_anterior = no.inicio;
        for (ordem, &f) in filhos.iter().enumerate() {
            let nf = a.no(f);
            assert_eq!(a.pai(f), Some(id));
            assert_eq!(a.ordem(f), ordem);
            assert_eq!(nf.chave.is_some(), no.tipo == Tipo::Objeto, "só item de objeto tem chave");
            let comeco_f = nf.chave.as_ref().map_or(nf.inicio, |c| c.start);
            assert!(comeco_f > fim_anterior.saturating_sub(1) && nf.fim < no.fim, "filho fora do pai");
            fim_anterior = nf.fim;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// A prancheta 8 e a 15

#[test]
fn analisa_o_json_da_prancheta_8() {
    let a = valido(EXEMPLO);
    conferir_arvore(&a, EXEMPLO);
    assert_eq!(a.total(), 19);
    let raiz = a.raiz();
    assert_eq!(a.no(raiz).tipo, Tipo::Objeto);
    assert_eq!(a.contagem(raiz).as_deref(), Some("5 chaves"));
    assert_eq!(a.nome(raiz, EXEMPLO), "raiz");
    assert_eq!(nomes_dos_filhos(&a, raiz, EXEMPLO), ["versao", "tema", "idioma", "conexoes", "exportacao"]);
    assert_eq!(tipos_dos_filhos(&a, raiz), [Tipo::Inteiro, Tipo::Texto, Tipo::Texto, Tipo::Lista, Tipo::Objeto]);

    let conexoes = filho(&a, raiz, 3);
    assert_eq!(a.contagem(conexoes).as_deref(), Some("2 itens"));
    assert_eq!(nomes_dos_filhos(&a, conexoes, EXEMPLO), ["0", "1"], "o nome de um item de lista é o índice");
    let item0 = filho(&a, conexoes, 0);
    assert_eq!(a.contagem(item0).as_deref(), Some("4 chaves"));
    assert_eq!(nomes_dos_filhos(&a, item0, EXEMPLO), ["nome", "arquivo", "tempo_limite", "somente_leitura"]);
    assert_eq!(tipos_dos_filhos(&a, item0), [Tipo::Texto, Tipo::Texto, Tipo::Inteiro, Tipo::Booleano]);
    assert_eq!(a.contagem(filho(&a, conexoes, 1)).as_deref(), Some("4 chaves"));
    let exportacao = filho(&a, raiz, 4);
    assert_eq!(a.contagem(exportacao).as_deref(), Some("3 chaves"));
    assert_eq!(nomes_dos_filhos(&a, exportacao, EXEMPLO), ["formato", "separador", "cabecalho"]);
    assert_eq!(a.contagem(filho(&a, item0, 0)), None, "valor simples não tem contagem");

    // os valores como estão escritos e como valem
    assert_eq!(a.trecho(filho(&a, item0, 2), EXEMPLO), "30");
    assert_eq!(a.valor_de_texto(filho(&a, item0, 0), EXEMPLO).as_deref(), Some("Produção"));
    assert_eq!(a.trecho(filho(&a, item0, 1), EXEMPLO), r#""C:\\dados\\exemplo.db""#);
    assert_eq!(a.valor_de_texto(filho(&a, item0, 1), EXEMPLO).as_deref(), Some(r"C:\dados\exemplo.db"));
    assert_eq!(a.valor_de_texto(filho(&a, item0, 2), EXEMPLO), None, "só os textos têm valor de texto");

    // o cursor da prancheta, na linha 9, coluna 25 (logo depois do 30): a migalha
    let byte = byte_da_linha_e_coluna(EXEMPLO, 9, 25);
    let migalha: Vec<String> = a.caminho_em(byte).into_iter().map(|n| a.nome(n, EXEMPLO)).collect();
    assert_eq!(migalha, ["raiz", "conexoes", "0", "tempo_limite"]);

    assert_eq!(EXEMPLO.lines().count(), 24);
    assert_eq!(formatar(EXEMPLO, Recuo::Espacos(2)).as_deref(), Ok(EXEMPLO), "o exemplo já está formatado");
}

#[test]
fn erro_da_prancheta_15_linha_14_coluna_7_diz_a_virgula() {
    // a mesma configuração, sem a vírgula depois do nome da segunda conexão
    let sem_virgula = EXEMPLO.replace("\"nome\": \"Relatórios\",", "\"nome\": \"Relatórios\"");
    let e = erro(&sem_virgula);
    assert_eq!((e.linha, e.coluna), (14, 7));
    assert_eq!(e.motivo, "falta uma vírgula ou um } antes de \"arquivo\"");
    assert_eq!(e.to_string(), "JSON inválido na linha 14, coluna 7: falta uma vírgula ou um } antes de \"arquivo\"");
    assert_eq!(&sem_virgula[e.byte..e.byte + 9], "\"arquivo\"", "o byte aponta o símbolo que estraga");
    // sem árvore: a tela guarda a última válida
    let a = analisar(&sem_virgula);
    assert!(a.arvore.is_none() && a.erro.is_some());
}

// ---------------------------------------------------------------------------------------------------------------
// Os motivos de erro

#[test]
fn um_teste_por_motivo_de_erro() {
    let tabela: [(&str, usize, usize, &str); 17] = [
        // 1
        ("{\"a\": 1\n \"b\": 2}", 2, 2, "falta uma vírgula ou um } antes de \"b\""),
        // 2
        ("[1 2]", 1, 4, "falta uma vírgula ou um ] antes de 2"),
        // 3
        ("{1: 2}", 1, 2, "esperava um nome entre aspas"),
        // 4
        ("{\"a\" 1}", 1, 6, "faltam os dois pontos depois de \"a\""),
        // 5
        ("{\"a\": }", 1, 7, "esperava um valor"),
        // 6
        ("[abc]", 1, 2, "valor inesperado: abc"),
        // 7
        ("[\"abc", 1, 2, "texto sem aspas de fechamento"),
        // 8
        ("[\"a\nb\"]", 1, 2, "o texto tem uma quebra de linha solta; escreva \\n"),
        // 9
        ("[\"a\\qb\"]", 1, 4, "escape inválido: \\q"),
        // 10
        ("[01]", 1, 2, "número inválido: 01"),
        // 11
        ("[1,]", 1, 3, "vírgula sobrando antes de } ou ]"),
        // 12
        ("{\"a\": [1, 2", 1, 12, "o documento termina antes de fechar { ou ["),
        // 13
        ("{} x", 1, 4, "há texto depois do fim do documento"),
        // 14
        ("[1, // c\n 2]", 1, 5, "JSON não aceita comentários"),
        // 15
        ("{'a': 1}", 1, 2, "use aspas duplas, não simples"),
        // 16
        ("[NaN]", 1, 2, "NaN e Infinity não existem em JSON"),
        // 17 (o acréscimo: a tabulação solta dentro do texto)
        ("[\"a\tb\"]", 1, 4, "o texto tem um caractere de controle solto; escreva-o como \\uXXXX"),
    ];
    for (texto, linha, coluna, motivo) in tabela {
        conferir(texto, linha, coluna, motivo);
    }
    // cada motivo é diferente dos outros
    let mut frases: Vec<&str> = tabela.iter().map(|t| t.3).collect();
    frases.sort_unstable();
    frases.dedup();
    assert_eq!(frases.len(), 17);
}

#[test]
fn variantes_de_cada_motivo() {
    // 1 e 2: o que veio no lugar da vírgula
    conferir("{\"a\": 1 2}", 1, 9, "falta uma vírgula ou um } antes de 2");
    conferir("{\"a\": 1 true}", 1, 9, "falta uma vírgula ou um } antes de true");
    conferir("{\"a\": 1 ]", 1, 9, "falta uma vírgula ou um } antes de ]");
    conferir("{\"a\": {} \"b\": 1}", 1, 10, "falta uma vírgula ou um } antes de \"b\"");
    conferir("[1 }", 1, 4, "falta uma vírgula ou um ] antes de }");
    conferir("[\"a\" \"b\"]", 1, 6, "falta uma vírgula ou um ] antes de \"b\"");
    conferir("[[] []]", 1, 5, "falta uma vírgula ou um ] antes de [");
    conferir("[1 {\"a\": 1}]", 1, 4, "falta uma vírgula ou um ] antes de {");
    conferir("[1 'x']", 1, 4, "falta uma vírgula ou um ] antes de '");
    // 3: nome
    conferir("{,}", 1, 2, "esperava um nome entre aspas");
    conferir("{\"a\": 1, 2}", 1, 10, "esperava um nome entre aspas");
    conferir("{a: 1}", 1, 2, "esperava um nome entre aspas");
    conferir("{[]: 1}", 1, 2, "esperava um nome entre aspas");
    conferir("{“a”: 1}", 1, 2, "esperava um nome entre aspas");
    conferir("{]", 1, 2, "esperava um nome entre aspas");
    // 4: dois pontos
    conferir("{\"a\"}", 1, 5, "faltam os dois pontos depois de \"a\"");
    conferir("{\"a\", 1}", 1, 5, "faltam os dois pontos depois de \"a\"");
    conferir("{\"a\\n\" = 1}", 1, 8, "faltam os dois pontos depois de \"a\\n\"");
    conferir("{\"a\"", 1, 5, "o documento termina antes de fechar { ou [");
    // 5: valor
    conferir("{\"a\":,}", 1, 6, "esperava um valor");
    conferir("{\"a\":}", 1, 6, "esperava um valor");
    conferir("[,1]", 1, 2, "esperava um valor");
    conferir("[1,,2]", 1, 4, "esperava um valor");
    conferir("[}", 1, 2, "esperava um valor");
    conferir("]", 1, 1, "esperava um valor");
    conferir("}", 1, 1, "esperava um valor");
    conferir(":", 1, 1, "esperava um valor");
    conferir("{\"a\": :}", 1, 7, "esperava um valor");
    conferir("", 1, 1, "esperava um valor");
    conferir("  \n\t ", 1, 1, "esperava um valor");
    // 6: valor inesperado
    conferir("[tru]", 1, 2, "valor inesperado: tru");
    conferir("[True]", 1, 2, "valor inesperado: True");
    conferir("[nul]", 1, 2, "valor inesperado: nul");
    conferir("[truex]", 1, 2, "valor inesperado: truex");
    conferir("[undefined]", 1, 2, "valor inesperado: undefined");
    conferir("[@]", 1, 2, "valor inesperado: @");
    conferir("[“x”]", 1, 2, "valor inesperado: “x”");
    conferir("[\u{a0}1]", 1, 2, "valor inesperado: U+00A0");
    conferir("{\"a\": #}", 1, 7, "valor inesperado: #");
    conferir("[/]", 1, 2, "valor inesperado: /");
    conferir("abc", 1, 1, "valor inesperado: abc");
    // 7: sem aspas de fechamento
    conferir("\"abc", 1, 1, "texto sem aspas de fechamento");
    conferir("{\"a\": \"abc", 1, 7, "texto sem aspas de fechamento");
    conferir("[\"abc\\", 1, 2, "texto sem aspas de fechamento");
    conferir("{\"abc", 1, 2, "texto sem aspas de fechamento");
    // 8: quebra de linha solta, na aspa de abertura (a mesma linha do texto)
    conferir("{\"a\": \"um\ndois\"}", 1, 7, "o texto tem uma quebra de linha solta; escreva \\n");
    conferir("{\"a\nb\": 1}", 1, 2, "o texto tem uma quebra de linha solta; escreva \\n");
    conferir("[\"a\rb\"]", 1, 2, "o texto tem uma quebra de linha solta; escreva \\n");
    conferir("[\"a\\\nb\"]", 1, 2, "o texto tem uma quebra de linha solta; escreva \\n");
    conferir("[1,\n \"a\nb\"]", 2, 2, "o texto tem uma quebra de linha solta; escreva \\n");
    // 9: escape
    conferir("[\"\\x41\"]", 1, 3, "escape inválido: \\x");
    conferir("[\"\\'\"]", 1, 3, "escape inválido: \\'");
    conferir("[\"\\U0041\"]", 1, 3, "escape inválido: \\U");
    conferir("[\"\\ç\"]", 1, 3, "escape inválido: \\ç");
    conferir("[\"\\u12\"]", 1, 3, "escape inválido: \\u12");
    conferir("[\"\\u12G4\"]", 1, 3, "escape inválido: \\u12G4");
    conferir("[\"\\u\"]", 1, 3, "escape inválido: \\u");
    conferir("[\"\\u00e\"]", 1, 3, "escape inválido: \\u00e");
    conferir("{\"a\\qb\": 1}", 1, 4, "escape inválido: \\q");
    // 10: número
    for (texto, mostrado) in [
        ("[-01]", "-01"),
        ("[00]", "00"),
        ("[1.]", "1."),
        ("[.5]", ".5"),
        ("[+1]", "+1"),
        ("[1e]", "1e"),
        ("[1e+]", "1e+"),
        ("[1.e5]", "1.e5"),
        ("[1.5.2]", "1.5.2"),
        ("[--1]", "--1"),
        ("[-]", "-"),
        ("[1_000]", "1_000"),
        ("[0x1F]", "0x1F"),
        ("[1e5.5]", "1e5.5"),
        ("[1ee5]", "1ee5"),
        ("[12abc]", "12abc"),
        ("[-a]", "-a"),
        ("[1-2]", "1-2"),
        ("[1.2e-]", "1.2e-"),
    ] {
        conferir(texto, 1, 2, &format!("número inválido: {mostrado}"));
    }
    conferir("{\"a\": 1.}", 1, 7, "número inválido: 1.");
    // 11: vírgula sobrando (aponta a vírgula)
    conferir("{\"a\": 1,}", 1, 8, "vírgula sobrando antes de } ou ]");
    conferir("[1,2,]", 1, 5, "vírgula sobrando antes de } ou ]");
    conferir("[1,}", 1, 3, "vírgula sobrando antes de } ou ]");
    conferir("{\"a\": [1,\n  ]}", 1, 9, "vírgula sobrando antes de } ou ]");
    conferir("{\"a\": 1,\n}", 1, 8, "vírgula sobrando antes de } ou ]");
    // 12: termina antes (aponta o fim do último símbolo)
    conferir("[", 1, 2, "o documento termina antes de fechar { ou [");
    conferir("{", 1, 2, "o documento termina antes de fechar { ou [");
    conferir("[1,", 1, 4, "o documento termina antes de fechar { ou [");
    conferir("{\"a\":", 1, 6, "o documento termina antes de fechar { ou [");
    conferir("{\"a\":1", 1, 7, "o documento termina antes de fechar { ou [");
    conferir("{\"a\": 1,", 1, 9, "o documento termina antes de fechar { ou [");
    conferir("[[1], [2]", 1, 10, "o documento termina antes de fechar { ou [");
    conferir("[1,\n  2\n\n", 2, 4, "o documento termina antes de fechar { ou [");
    conferir("{\n  \"a\": [\n", 2, 9, "o documento termina antes de fechar { ou [");
    // 13: texto depois do fim
    conferir("{}}", 1, 3, "há texto depois do fim do documento");
    conferir("1 2", 1, 3, "há texto depois do fim do documento");
    conferir("\"a\" \"b\"", 1, 5, "há texto depois do fim do documento");
    conferir("{}{}", 1, 3, "há texto depois do fim do documento");
    conferir("[1]\n\nlixo", 3, 1, "há texto depois do fim do documento");
    conferir("null,", 1, 5, "há texto depois do fim do documento");
    conferir("{\"a\":1}/", 1, 8, "há texto depois do fim do documento");
    // 14: comentário
    conferir("// c\n{}", 1, 1, "JSON não aceita comentários");
    conferir("/* c */ {}", 1, 1, "JSON não aceita comentários");
    conferir("{} // c", 1, 4, "JSON não aceita comentários");
    conferir("[1 /* c */]", 1, 4, "JSON não aceita comentários");
    conferir("{\"a\": /* c */ 1}", 1, 7, "JSON não aceita comentários");
    conferir("{\n  // a configuração\n  \"a\": 1\n}", 2, 3, "JSON não aceita comentários");
    conferir("{\"a\" // c\n: 1}", 1, 6, "JSON não aceita comentários");
    // 15: aspas simples
    conferir("['a']", 1, 2, "use aspas duplas, não simples");
    conferir("{\"a\": 'x'}", 1, 7, "use aspas duplas, não simples");
    conferir("'x'", 1, 1, "use aspas duplas, não simples");
    conferir("{\"a\": 1, 'b': 2}", 1, 10, "use aspas duplas, não simples");
    // 16: NaN e Infinity
    for texto in ["[NaN]", "[Infinity]", "[-Infinity]"] {
        let e = erro(texto);
        assert_eq!((e.linha, e.coluna, e.motivo.as_str()), (1, 2, "NaN e Infinity não existem em JSON"), "{texto}");
    }
    conferir("{\"a\": NaN}", 1, 7, "NaN e Infinity não existem em JSON");
    conferir("Infinity", 1, 1, "NaN e Infinity não existem em JSON");
    // 17: controle solto
    conferir("[\"\u{1}\"]", 1, 3, "o texto tem um caractere de controle solto; escreva-o como \\uXXXX");
    conferir("{\"a\tb\": 1}", 1, 4, "o texto tem um caractere de controle solto; escreva-o como \\uXXXX");
    conferir("[\"a\\\u{1}\"]", 1, 5, "o texto tem um caractere de controle solto; escreva-o como \\uXXXX");
}

#[test]
fn trechos_compridos_sao_cortados_na_frase() {
    let comprido = "a".repeat(100);
    conferir(&format!("[{comprido}]"), 1, 2, &format!("valor inesperado: {}…", "a".repeat(30)));
    conferir(
        &format!("[1 \"{comprido}\"]"),
        1,
        4,
        &format!("falta uma vírgula ou um ] antes de \"{}…\"", "a".repeat(30)),
    );
    conferir(
        &format!("{{\"{}\" 1}}", "b".repeat(100)),
        1,
        105,
        &format!("faltam os dois pontos depois de \"{}…\"", "b".repeat(40)),
    );
    conferir(&format!("[1{}]", "9".repeat(50) + "x"), 1, 2, &format!("número inválido: 1{}…", "9".repeat(29)));
    // ao cortar, nunca no meio de um caractere
    conferir(&format!("[{}]", "ç".repeat(40)), 1, 2, &format!("valor inesperado: {}…", "ç".repeat(30)));
}

#[test]
fn rejeita_comentario_aspa_simples_nan_e_virgula_sobrando() {
    for texto in [
        "// x\n[]",
        "/* x */[]",
        "[] // x",
        "[1, /* x */ 2]",
        "{\"a\": 1, // x\n \"b\": 2}",
        "{/* x */}",
        "[/* x */]",
        "/**/[]",
        "/",
    ] {
        let e = erro(texto);
        if texto == "/" {
            assert_eq!(e.motivo, "valor inesperado: /", "uma barra sozinha não é comentário");
        } else {
            assert_eq!(e.motivo, "JSON não aceita comentários", "{texto:?}");
        }
    }
    for texto in ["['a']", "{'a': 1}", "{\"a\": 'b'}", "'a'", "[1, 'x']"] {
        assert_eq!(erro(texto).motivo, "use aspas duplas, não simples", "{texto}");
    }
    for texto in ["NaN", "Infinity", "-Infinity", "[NaN, 1]", "{\"a\": Infinity}", "[1, -Infinity]"] {
        assert_eq!(erro(texto).motivo, "NaN e Infinity não existem em JSON", "{texto}");
    }
    for texto in ["[1,]", "{\"a\": 1,}", "[1, 2, ]", "{\"a\": [1,],}", "[[1,],]", "{\"a\": {\"b\": 2,}}"] {
        assert_eq!(erro(texto).motivo, "vírgula sobrando antes de } ou ]", "{texto}");
    }
    // o que se parece, mas é texto, passa
    valido("[\"//\", \"/* x */\", \"'\", \"NaN\", \"Infinity\", \",]\", \"{'a': 1}\"]");
    valido("{\"//\": \"/*\", \"'a'\": \"NaN\"}");
}

// ---------------------------------------------------------------------------------------------------------------
// O que é aceito

#[test]
fn aceita_chave_repetida_e_guarda_as_duas() {
    let texto = "{\"a\": 1, \"b\": 0, \"a\": 2, \"a\": \"três\"}";
    let a = valido(texto);
    let raiz = a.raiz();
    assert_eq!(a.no(raiz).filhos, 4);
    assert_eq!(nomes_dos_filhos(&a, raiz, texto), ["a", "b", "a", "a"]);
    let valores: Vec<&str> = a.filhos(raiz).map(|f| a.trecho(f, texto)).collect();
    assert_eq!(valores, ["1", "0", "2", "\"três\""]);
    conferir_arvore(&a, texto);
}

#[test]
fn numeros_com_expoente_e_zero_a_esquerda() {
    for (numero, tipo) in [
        ("0", Tipo::Inteiro),
        ("-0", Tipo::Inteiro),
        ("1", Tipo::Inteiro),
        ("-1", Tipo::Inteiro),
        ("10", Tipo::Inteiro),
        ("123456789012345678901234567890", Tipo::Inteiro),
        ("1.5", Tipo::Numero),
        ("-1.5", Tipo::Numero),
        ("0.5", Tipo::Numero),
        ("0.000001", Tipo::Numero),
        ("1e5", Tipo::Numero),
        ("1E5", Tipo::Numero),
        ("1e+5", Tipo::Numero),
        ("1e-5", Tipo::Numero),
        ("1.5e10", Tipo::Numero),
        ("1.5E-10", Tipo::Numero),
        ("0e0", Tipo::Numero),
        ("-0.0e-0", Tipo::Numero),
        ("1.0", Tipo::Numero),
        ("1e999", Tipo::Numero),
    ] {
        for texto in [numero.to_string(), format!("[{numero}]"), format!("{{\"n\":{numero}}}"), format!(" {numero}\n")]
        {
            let a = valido(&texto);
            let no = if a.no(a.raiz()).tipo.e_container() { filho(&a, a.raiz(), 0) } else { a.raiz() };
            assert_eq!(a.no(no).tipo, tipo, "{texto:?}");
            assert_eq!(a.trecho(no, &texto), numero, "o texto do número fica como está");
        }
    }
    // zero à esquerda e companhia, em todas as posições
    for numero in ["01", "-01", "007", "00", "0123", "1.", ".5", "1.e2", "+1", "1e", "0x10"] {
        for texto in [numero.to_string(), format!("[{numero}]"), format!("{{\"n\": {numero}}}")] {
            assert!(erro(&texto).motivo.starts_with("número inválido: "), "{texto:?}");
        }
    }
}

#[test]
fn escapes_e_u_hexadecimal() {
    let casos: [(&str, &str); 14] = [
        (r#""\"\\\/\b\f\n\r\t""#, "\"\\/\u{8}\u{c}\n\r\t"),
        (r#""\u00e7""#, "ç"),
        (r#""\u00E7""#, "ç"),
        (r#""\u0041\u0042""#, "AB"),
        (r#""\ud83d\ude00""#, "😀"),
        (r#""\uD83D\uDE00""#, "😀"),
        (r#""\u20ac""#, "€"),
        (r#""\u0000""#, "\0"),
        (r#""a\u0020b""#, "a b"),
        (r#""\ud800""#, "\u{FFFD}"),
        (r#""\udc00""#, "\u{FFFD}"),
        (r#""\ud83dx""#, "\u{FFFD}x"),
        (r#""\ud83d\u0041""#, "\u{FFFD}A"),
        ("\"ação \u{1f600} \u{7f} \u{2028}\"", "ação \u{1f600} \u{7f} \u{2028}"),
    ];
    for (literal, esperado) in casos {
        let a = valido(literal);
        assert_eq!(a.valor_de_texto(a.raiz(), literal).as_deref(), Some(esperado), "{literal}");
        // a mesma cadeia como chave
        let objeto = format!("{{{literal}: 1}}");
        let b = valido(&objeto);
        assert_eq!(b.nome(filho(&b, b.raiz(), 0), &objeto), esperado, "{literal} como chave");
    }
    // desescapar direto
    assert_eq!(desescapar("sem escape"), "sem escape");
    assert_eq!(desescapar(r"a\nb"), "a\nb");
    assert_eq!(desescapar(r"\u12"), r"\u12", "malformado: fica como está");
    assert_eq!(desescapar(r"\q\"), r"\q\");
}

#[test]
fn valores_na_raiz_e_espacos_em_volta() {
    for (texto, tipo) in [
        ("3", Tipo::Inteiro),
        ("-2.5", Tipo::Numero),
        ("\"x\"", Tipo::Texto),
        ("null", Tipo::Nulo),
        ("true", Tipo::Booleano),
        ("false", Tipo::Booleano),
        ("[]", Tipo::Lista),
        ("{}", Tipo::Objeto),
        ("  \t\r\n {} \n\n", Tipo::Objeto),
        ("\u{FEFF}{\"a\": 1}", Tipo::Objeto),
    ] {
        let a = valido(texto);
        assert_eq!(a.no(a.raiz()).tipo, tipo, "{texto:?}");
        conferir_arvore(&a, texto);
    }
    let a = valido("  [1]  ");
    assert_eq!((a.no(a.raiz()).inicio, a.no(a.raiz()).fim), (2, 5));
    // o BOM é pulado, mas não muda os bytes
    let com_bom = "\u{FEFF}[1]";
    assert_eq!(valido(com_bom).no(NoId(0)).inicio, 3);
    // um texto fora do JSON depois do BOM ainda dá erro, no byte certo
    assert_eq!(erro("\u{FEFF}[1] x").byte, 7);
}

#[test]
fn posicoes_contam_caracteres_e_linhas() {
    // acento: 2 bytes, 1 coluna
    let e = erro("[\"ação\", é]");
    assert_eq!((e.byte, e.linha, e.coluna), (11, 1, 10));
    // a tabulação conta 1
    assert_eq!((erro("[\t1 2]").linha, erro("[\t1 2]").coluna), (1, 5));
    // linha de CRLF: o \r é da linha de cima
    let e = erro("{\r\n  \"a\": 1\r\n  \"b\": 2\r\n}");
    assert_eq!((e.linha, e.coluna), (3, 3));
    assert_eq!(e.motivo, "falta uma vírgula ou um } antes de \"b\"");
    // emoji fora do plano básico: 4 bytes, 1 coluna
    let e = erro("[\"😀😀\", ?]");
    assert_eq!((e.byte, e.coluna), (13, 8));
    // o erro de um documento sem erro na primeira linha
    let e = erro("{\n\n\n\"a\": tru}");
    assert_eq!((e.linha, e.coluna, e.motivo.as_str()), (4, 6, "valor inesperado: tru"));
}

// ---------------------------------------------------------------------------------------------------------------
// Profundidade e tamanho

/// Roda numa thread de pilha pequena: se alguma coisa fosse recursiva, 100.000 níveis a derrubariam.
fn com_pilha_pequena<F: FnOnce() + Send + 'static>(f: F) {
    thread::Builder::new().stack_size(256 * 1024).spawn(f).unwrap().join().unwrap();
}

#[test]
fn profundidade_de_100_mil_niveis_sem_recursao() {
    com_pilha_pequena(|| {
        const N: usize = 100_000;
        // listas aninhadas
        let listas = format!("{}{}", "[".repeat(N), "]".repeat(N));
        let a = valido(&listas);
        assert_eq!(a.total(), N);
        assert_eq!(a.no(a.raiz()).filhos, 1);
        // o caminho até o fundo (todas as listas) e o byte do meio
        assert_eq!(a.caminho_em(N - 1).len(), N);
        assert_eq!(a.caminho_em(N).len(), N);
        assert_eq!(a.caminho_em(N / 2).len(), N / 2 + 1);
        assert!(validar(&listas).is_none());
        drop(a); // soltar a árvore também não recursa

        // objetos aninhados
        let objetos = format!("{}1{}", "{\"a\":".repeat(N), "}".repeat(N));
        let a = valido(&objetos);
        assert_eq!(a.total(), N + 1);
        assert!(validar(&objetos).is_none());
        let folha = NoId(u32::try_from(N).unwrap());
        assert_eq!(a.no(folha).tipo, Tipo::Inteiro);
        assert_eq!(a.nome(folha, &objetos), "a");
        assert_eq!(a.pai(folha), Some(NoId(u32::try_from(N - 1).unwrap())));

        // misturados
        let misto = format!("{}null{}", "[{\"k\":".repeat(N / 2), "}]".repeat(N / 2));
        assert_eq!(valido(&misto).total(), N + 1);

        // sem fechar: o erro de sempre, sem estourar nada
        let aberto = "[".repeat(N);
        let e = erro(&aberto);
        assert_eq!(e.motivo, "o documento termina antes de fechar { ou [");
        assert_eq!((e.linha, e.coluna), (1, N + 1));
        let aberto = "{\"a\":".repeat(N);
        assert_eq!(validar(&aberto).map(|e| e.motivo).as_deref(), Some("o documento termina antes de fechar { ou ["));

        // fechado de menos e de mais
        let e = erro(&format!("{}{}", "[".repeat(N), "]".repeat(N + 1)));
        assert_eq!(e.motivo, "há texto depois do fim do documento");
        assert_eq!(e.byte, 2 * N);
    });
}

#[test]
fn formatar_fundo_nao_recursa_e_o_recuo_para_de_crescer() {
    com_pilha_pequena(|| {
        const N: usize = 20_000;
        let fundo = format!("{}1{}", "[".repeat(N), "]".repeat(N));
        let f = formatar(&fundo, Recuo::Espacos(2)).unwrap();
        // um valor por linha, e o recuo da última linha de abertura é o do nível máximo
        let linhas: Vec<&str> = f.lines().collect();
        assert_eq!(linhas.len(), 2 * N + 1);
        assert_eq!(linhas[0], "[");
        assert_eq!(linhas[1], "  [");
        let maximo = " ".repeat(2 * NIVEL_MAXIMO_DO_RECUO);
        assert_eq!(linhas[N - 1], format!("{maximo}[")); // abre o N-ésimo nível, já no recuo máximo
        assert_eq!(linhas[N], format!("{maximo}1"));
        assert_eq!(*linhas.last().unwrap(), "]");
        assert!(f.len() < fundo.len() * (2 + 2 * NIVEL_MAXIMO_DO_RECUO), "{} bytes", f.len());
        // a leitura do resultado é a mesma e formatar de novo não muda nada
        assert_eq!(compacto(&f), fundo);
        assert_eq!(formatar(&f, Recuo::Espacos(2)).unwrap(), f);
    });
}

#[test]
fn arquivo_grande_funciona() {
    // uns 2 MiB de JSON de verdade: valida, monta, formata, e o resultado é o mesmo documento
    let registro =
        r#"{"id":12345,"nome":"Fulano de Tal","ativo":true,"saldo":1234.56,"tags":["a","b","ç"],"obs":null,"fim":{}}"#;
    let n = 2 * 1024 * 1024 / (registro.len() + 1);
    let texto = format!("[{}]", vec![registro; n].join(","));
    let inicio = Instant::now();
    let a = valido(&texto);
    assert_eq!(a.no(a.raiz()).filhos as usize, n);
    assert_eq!(a.total(), 1 + n * 11, "o registro tem 11 valores (o objeto, 7 chaves e 3 itens de lista)");
    eprintln!("{} bytes analisados em {:?} ({} nós)", texto.len(), inicio.elapsed(), a.total());
    let f = formatar(&texto, Recuo::Espacos(4)).unwrap();
    assert_eq!(compacto(&f), texto);
    assert!(f.contains("\n    {\n        \"id\": 12345,\n"));
    assert_eq!(formatar(&f, Recuo::Espacos(4)).unwrap(), f);
    // a busca do byte no meio do arquivo
    let meio = texto.len() / 2;
    let caminho = a.caminho_em(meio);
    assert!(caminho.len() >= 2 && caminho[0] == a.raiz());
    // erro no fim do arquivo grande: linha e coluna certas
    let quebrado = format!("{}\n  x", &texto[..texto.len() - 1]);
    let e = erro(&quebrado);
    assert_eq!((e.linha, e.coluna), (2, 3));
}

// ---------------------------------------------------------------------------------------------------------------
// A estrutura: caminho, nomes, páginas

#[test]
fn caminho_em_um_byte_acha_a_migalha() {
    let a = valido(EXEMPLO);
    let migalha =
        |byte: usize| -> Vec<String> { a.caminho_em(byte).into_iter().skip(1).map(|n| a.nome(n, EXEMPLO)).collect() };
    let em = |linha: usize, coluna: usize| byte_da_linha_e_coluna(EXEMPLO, linha, coluna);
    // a chave, o valor, logo depois do valor, a vírgula
    assert_eq!(migalha(em(9, 10)), ["conexoes", "0", "tempo_limite"], "dentro da chave");
    assert_eq!(migalha(em(9, 7)), ["conexoes", "0", "tempo_limite"], "na aspa da chave");
    assert_eq!(migalha(em(9, 23)), ["conexoes", "0", "tempo_limite"], "no começo do 30");
    assert_eq!(migalha(em(9, 25)), ["conexoes", "0", "tempo_limite"], "logo depois do 30 (o cursor da prancheta)");
    assert_eq!(migalha(em(9, 26)), ["conexoes", "0"], "depois da vírgula, no espaço entre os itens, vale o objeto");
    // nos recuos e nas linhas dos delimitadores
    assert_eq!(migalha(em(9, 3)), ["conexoes", "0"], "no recuo da linha");
    assert_eq!(migalha(em(6, 5)), ["conexoes", "0"], "no {{ do item");
    assert_eq!(migalha(em(11, 5)), ["conexoes", "0"], "no }} do item");
    assert_eq!(migalha(em(5, 14)), ["conexoes"], "entre os dois pontos e o [ da lista");
    assert_eq!(migalha(em(5, 15)), ["conexoes"], "no [ da lista");
    assert_eq!(migalha(em(15, 8)), ["conexoes", "1", "tempo_limite"]);
    assert_eq!(migalha(em(21, 20)), ["exportacao", "separador"]);
    assert_eq!(migalha(em(2, 3)), ["versao"]);
    // a raiz sozinha: o { e o } do documento
    assert_eq!(a.caminho_em(0), [a.raiz()]);
    assert_eq!(migalha(0), Vec::<String>::new());
    assert_eq!(a.caminho_em(EXEMPLO.len() - 2), [a.raiz()], "no }} final");
    // depois do fim do documento (a quebra final), nenhum nó
    assert_eq!(a.caminho_em(EXEMPLO.len()), []);
    assert_eq!(a.caminho_em(usize::MAX), []);

    // os espaços de fora do documento não são de nó nenhum
    let texto = "  [1]  ";
    let b = valido(texto);
    assert_eq!(b.caminho_em(0), []);
    assert_eq!(b.caminho_em(1), []);
    assert_eq!(b.caminho_em(2), [b.raiz()]);
    assert_eq!(b.caminho_em(3).len(), 2);
    assert_eq!(b.caminho_em(4).len(), 2, "logo depois do 1");
    assert_eq!(b.caminho_em(5), [b.raiz()], "logo depois do ]");
    assert_eq!(b.caminho_em(6), []);

    // valor simples na raiz
    let c = valido("42");
    assert_eq!(c.caminho_em(1), [c.raiz()]);
    assert_eq!(c.caminho_em(2), [c.raiz()]);
}

/// O caminho de um byte, achado do jeito lento: o nó mais fundo que o contém.
fn caminho_lento(a: &Arvore, byte: usize) -> Vec<NoId> {
    let contem = |id: NoId| {
        let no = a.no(id);
        let comeco = no.chave.as_ref().map_or(no.inicio, |c| c.start);
        comeco <= byte && byte <= no.fim
    };
    let mais_fundo = (0..a.total()).map(|i| NoId(u32::try_from(i).unwrap())).rfind(|&id| contem(id));
    let mut caminho: Vec<NoId> = mais_fundo.into_iter().collect();
    while let Some(pai) = caminho.last().and_then(|&n| a.pai(n)) {
        caminho.push(pai);
    }
    caminho.reverse();
    caminho
}

#[test]
fn caminho_em_confere_com_a_busca_lenta_em_todo_byte() {
    for texto in
        [EXEMPLO, COMPACTO, "  [ 1 , [ 2 , { \"a\" : [ ] } ] , { } ]  ", "{\"a\":{\"b\":{\"c\":[1,2,3]}}}", "7"]
    {
        let a = valido(texto);
        for byte in 0..=texto.len() + 1 {
            assert_eq!(a.caminho_em(byte), caminho_lento(&a, byte), "byte {byte} de {texto:?}");
        }
    }
}

#[test]
fn nome_pagina_e_contagens() {
    let a = valido(r#"{"a\"b":1,"\u00e7":[],"z":{"k":0}}"#);
    let t = r#"{"a\"b":1,"\u00e7":[],"z":{"k":0}}"#;
    assert_eq!(nomes_dos_filhos(&a, a.raiz(), t), ["a\"b", "ç", "z"], "a chave sai com os escapes desfeitos");
    assert_eq!(a.contagem(filho(&a, a.raiz(), 1)).as_deref(), Some("0 itens"));
    assert_eq!(a.contagem(filho(&a, a.raiz(), 2)).as_deref(), Some("1 chave"));
    let lista = valido("[1]");
    assert_eq!(lista.contagem(lista.raiz()).as_deref(), Some("1 item"));
    assert_eq!(valido("{}").contagem(NoId(0)).as_deref(), Some("0 chaves"));

    // 450 itens: páginas de 200, 200 e 50
    let texto = format!("[{}]", (0..450).map(|i| i.to_string()).collect::<Vec<_>>().join(","));
    let a = valido(&texto);
    assert_eq!(FILHOS_POR_PAGINA, 200);
    let p1 = a.pagina_de_filhos(a.raiz(), 0);
    assert_eq!((p1.nos.len(), p1.restantes), (200, 250));
    assert_eq!(a.nome(p1.nos[199], &texto), "199");
    let p2 = a.pagina_de_filhos(a.raiz(), 200);
    assert_eq!((p2.nos.len(), p2.restantes), (200, 50));
    assert_eq!(a.nome(p2.nos[0], &texto), "200");
    let p3 = a.pagina_de_filhos(a.raiz(), 400);
    assert_eq!((p3.nos.len(), p3.restantes), (50, 0));
    assert_eq!(a.nome(p3.nos[49], &texto), "449");
    let alem = a.pagina_de_filhos(a.raiz(), 9999);
    assert_eq!((alem.nos.len(), alem.restantes), (0, 0));
    // exatamente 200: sem "mais"
    let t200 = format!("[{}]", vec!["0"; 200].join(","));
    let a200 = valido(&t200);
    let p = a200.pagina_de_filhos(a200.raiz(), 0);
    assert_eq!((p.nos.len(), p.restantes), (200, 0));
    // um valor simples não tem filhos
    assert_eq!(a.pagina_de_filhos(p1.nos[0], 0).nos, []);
    // a ordem do item é o índice
    assert_eq!(a.ordem(p2.nos[5]), 205);

    // os textos
    assert_eq!(mais_itens_em_texto(1800), "mais 1.800 itens");
    assert_eq!(mais_itens_em_texto(250), "mais 250 itens");
    assert_eq!(mais_itens_em_texto(1), "mais 1 item");
    assert_eq!(mais_itens_em_texto(1_234_567), "mais 1.234.567 itens");
    assert_eq!(milhares(0), "0");
    assert_eq!(milhares(999), "999");
    assert_eq!(milhares(1000), "1.000");
    assert_eq!(milhares(100_000), "100.000");
}

#[test]
fn validar_concorda_com_analisar() {
    for texto in [EXEMPLO, COMPACTO, "[1,]", "", "{\"a\" 1}", "[[[", "\"x\"", "tru"] {
        assert_eq!(validar(texto), analisar(texto).erro, "{texto:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Formatar

#[test]
fn formatar_preserva_a_ordem_das_chaves() {
    let f = formatar(r#"{"z":1,"a":2,"m":{"y":0,"b":1},"a":3}"#, Recuo::Espacos(2)).unwrap();
    assert_eq!(f, "{\n  \"z\": 1,\n  \"a\": 2,\n  \"m\": {\n    \"y\": 0,\n    \"b\": 1\n  },\n  \"a\": 3\n}");
    // a lista de chaves, na ordem, antes e depois
    let a = valido(&f);
    assert_eq!(nomes_dos_filhos(&a, a.raiz(), &f), ["z", "a", "m", "a"]);
}

#[test]
fn formatar_layout_dos_recuos_vazios_e_quebra_final() {
    let compacto_ = r#"{"a":1,"b":[1,2,{"c":null}],"d":{},"e":[],"f":[[],{}]}"#;
    let dois = "{\n  \"a\": 1,\n  \"b\": [\n    1,\n    2,\n    {\n      \"c\": null\n    }\n  ],\n  \"d\": {},\n  \"e\": [],\n  \"f\": [\n    [],\n    {}\n  ]\n}";
    assert_eq!(formatar(compacto_, Recuo::Espacos(2)).unwrap(), dois);
    let quatro = dois.replace("  ", "    ");
    assert_eq!(formatar(compacto_, Recuo::Espacos(4)).unwrap(), quatro);
    let tabs = dois.replace("  ", "\t");
    assert_eq!(formatar(compacto_, Recuo::Tabulacao).unwrap(), tabs);
    assert_eq!(formatar(compacto_, Recuo::Espacos(0)).unwrap(), dois.replace("  ", ""));

    // a quebra final se mantém (se havia, há; se não, não), e CRLF vira \n
    assert!(!formatar("[1]", Recuo::Espacos(2)).unwrap().ends_with('\n'));
    assert!(formatar("[1]\n", Recuo::Espacos(2)).unwrap().ends_with("]\n"));
    assert!(formatar("[1]\r\n", Recuo::Espacos(2)).unwrap().ends_with("]\n"));
    assert!(formatar("[1]  \n\n\n", Recuo::Espacos(2)).unwrap().ends_with("]\n"), "uma só");
    assert_eq!(formatar("[1]  ", Recuo::Espacos(2)).unwrap(), "[\n  1\n]");
    assert!(!formatar("[1]\r\n", Recuo::Espacos(2)).unwrap().contains('\r'));
    // vazios com espaço por dentro ficam juntos
    assert_eq!(formatar("[ ]", Recuo::Espacos(2)).unwrap(), "[]");
    assert_eq!(formatar("{\n\n}", Recuo::Espacos(2)).unwrap(), "{}");
    assert_eq!(formatar("[ { } , [ \n ] ]", Recuo::Espacos(2)).unwrap(), "[\n  {},\n  []\n]");
    // valor simples na raiz
    assert_eq!(formatar("  42 \n", Recuo::Espacos(2)).unwrap(), "42\n");
    assert_eq!(formatar("\"x\"", Recuo::Espacos(2)).unwrap(), "\"x\"");
    // BOM no começo não vai para o resultado
    assert_eq!(formatar("\u{FEFF}[]", Recuo::Espacos(2)).unwrap(), "[]");
    // dois pontos sempre com um espaço depois e nenhum antes
    assert_eq!(formatar("{\"a\"   :   1}", Recuo::Espacos(1)).unwrap(), "{\n \"a\": 1\n}");
}

#[test]
fn formatar_nao_mexe_nos_numeros_nem_nas_cadeias() {
    let entrada = r#"{"n":[1.0,1e5,-0,1E+2,0.10,100000000000000000000,-0.0e-0,12345678901234567890.5],"s":"a\u00e7\n\/\"  x   y","e":"","t":"  ","u":"ação \ud83d\ude00","k\u00e7\"":"v"}"#;
    let f = formatar(entrada, Recuo::Espacos(2)).unwrap();
    let a = valido(&f);
    let b = valido(entrada);
    // os mesmos valores, no mesmo texto, na mesma ordem
    let valores = |arv: &Arvore, t: &str| -> Vec<(String, String)> {
        (0..arv.total())
            .map(|i| NoId(u32::try_from(i).unwrap()))
            .filter(|&id| !arv.no(id).tipo.e_container())
            .map(|id| (arv.nome(id, t), arv.trecho(id, t).to_string()))
            .collect()
    };
    assert_eq!(valores(&a, &f), valores(&b, entrada));
    for numero in ["1.0", "1e5", "-0", "1E+2", "0.10", "100000000000000000000", "-0.0e-0", "12345678901234567890.5"] {
        assert!(f.contains(&format!("    {numero}")), "{numero} saiu mudado:\n{f}");
    }
    for cadeia in [r#""a\u00e7\n\/\"  x   y""#, r#""  ""#, r#""ação \ud83d\ude00""#, r#""k\u00e7\"""#] {
        assert!(f.contains(cadeia), "{cadeia} saiu mudada:\n{f}");
    }
    assert_eq!(compacto(&f), entrada, "só os espaços e quebras de fora das cadeias mudaram");
    // chave repetida continua repetida
    assert_eq!(formatar("{\"a\":1,\"a\":1}", Recuo::Espacos(2)).unwrap(), "{\n  \"a\": 1,\n  \"a\": 1\n}");
}

#[test]
fn formatar_e_idempotente() {
    for texto in
        [EXEMPLO, COMPACTO, "[]", "{}", "1", "{\"a\":[[[]]]}", "[1,\n\n2,   3\t,4]\n", "  {\"a\"\n:\n[\n]\n}\n\n"]
    {
        for recuo in [Recuo::Espacos(2), Recuo::Espacos(4), Recuo::Tabulacao, Recuo::Espacos(1)] {
            let uma = formatar(texto, recuo).unwrap();
            let duas = formatar(&uma, recuo).unwrap();
            assert_eq!(uma, duas, "{texto:?} com {recuo:?}");
            assert_eq!(compacto(&uma), compacto(texto), "o conteúdo não pode mudar");
            let a = valido(&uma);
            conferir_arvore(&a, &uma);
            assert_eq!(a.total(), valido(texto).total());
        }
    }
}

#[test]
fn formatar_json_invalido_devolve_o_erro() {
    let e = formatar("{\"a\": 1,}", Recuo::Espacos(2)).unwrap_err();
    assert_eq!((e.linha, e.coluna, e.motivo.as_str()), (1, 8, "vírgula sobrando antes de } ou ]"));
    assert_eq!(formatar("", Recuo::Espacos(2)).unwrap_err().motivo, "esperava um valor");
    assert_eq!(formatar("[1 2]", Recuo::Tabulacao).unwrap_err(), erro("[1 2]"));
    assert_eq!(
        formatar("// x\n{}", Recuo::Espacos(2)).unwrap_err().motivo,
        "JSON não aceita comentários",
        "formatar não descarta comentário"
    );
}

#[test]
fn detectar_recuo_dois_quatro_tabulacao_e_nenhum() {
    assert_eq!(detectar_recuo(EXEMPLO), Recuo::Espacos(2));
    assert_eq!(recuo_do_texto(EXEMPLO), Some(Recuo::Espacos(2)));
    assert_eq!(detectar_recuo("{\n    \"a\": 1\n}"), Recuo::Espacos(4));
    assert_eq!(detectar_recuo("{\n\t\"a\": 1\n}"), Recuo::Tabulacao);
    assert_eq!(detectar_recuo("{\n   \"a\": 1\n}"), Recuo::Espacos(3));
    assert_eq!(detectar_recuo("{\r\n    \"a\": 1\r\n}"), Recuo::Espacos(4), "CRLF");
    // nenhum recuo: 2 espaços para formatar, e `None` para o rodapé dizer "Sem recuo"
    for sem in ["", "{}", "[1,2,3]", "{\"a\":1}\n", "{\n\"a\": 1\n}", "\n\n  \n\t\n"] {
        assert_eq!(detectar_recuo(sem), Recuo::Espacos(2), "{sem:?}");
        assert_eq!(recuo_do_texto(sem), None, "{sem:?}");
    }
    // a primeira linha recuada manda; linha em branco com espaços não conta
    assert_eq!(detectar_recuo("[\n    \n  1\n]"), Recuo::Espacos(2));
    assert_eq!(detectar_recuo("[\n\t1,\n    2\n]"), Recuo::Tabulacao);
    assert_eq!(detectar_recuo("[\n  1,\n\t2\n]"), Recuo::Espacos(2));
    // recuo enorme: limitado a 8
    assert_eq!(detectar_recuo(&format!("[\n{}1\n]", " ".repeat(30))), Recuo::Espacos(8));
    // os rótulos do rodapé
    assert_eq!(Recuo::Espacos(2).rotulo(), "Espaços: 2");
    assert_eq!(Recuo::Espacos(4).rotulo(), "Espaços: 4");
    assert_eq!(Recuo::Tabulacao.rotulo(), "Tabulação");
}

// ---------------------------------------------------------------------------------------------------------------
// Texto mutilado: nada pode dar pânico, e o que sai tem de fazer sentido

/// Um gerador de números repetível (xorshift).
struct Sorteio(u64);

impl Sorteio {
    fn proximo(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn ate(&mut self, n: usize) -> usize {
        usize::try_from(self.proximo() % n as u64).unwrap()
    }
}

/// Uma versão do texto com um pedaço apagado, trocado ou acrescentado.
fn mutar(texto: &str, s: &mut Sorteio) -> String {
    const PECAS: [&str; 22] = [
        "\"", "\\", ",", ":", "{", "}", "[", "]", " ", "\n", "\t", "1", "-", "e", ".", "/", "'", "ç", "😀", "\0",
        "tru", "\\u12",
    ];
    let mut t = texto.to_string();
    for _ in 0..=s.ate(3) {
        let a = t.floor_char_boundary(s.ate(t.len() + 1));
        let b = t.ceil_char_boundary((a + s.ate(4)).min(t.len()));
        match s.ate(3) {
            0 => t.replace_range(a..b, ""),
            1 => t.replace_range(a..b, PECAS[s.ate(PECAS.len())]),
            _ => t.insert_str(a, PECAS[s.ate(PECAS.len())]),
        }
    }
    t
}

fn conferir_analise_de_texto_qualquer(t: &str) {
    let a = analisar(t);
    let v = validar(t);
    assert_eq!(a.erro, v, "validar e analisar discordam em {t:?}");
    assert_ne!(a.arvore.is_some(), a.erro.is_some(), "ou árvore ou erro, em {t:?}");
    match (&a.arvore, &a.erro) {
        (None, Some(e)) => {
            assert!(e.byte <= t.len() && t.is_char_boundary(e.byte), "byte {} fora de lugar em {t:?}", e.byte);
            assert_eq!((e.linha, e.coluna), linha_e_coluna(t, e.byte), "{t:?}");
            assert!(!e.motivo.is_empty() && e.to_string().starts_with("JSON inválido na linha "), "{t:?}");
            assert_eq!(formatar(t, Recuo::Espacos(2)).unwrap_err(), *e);
        }
        (Some(arvore), None) => {
            conferir_arvore(arvore, t);
            // o resultado do formatar é o mesmo documento, e formatar de novo não muda nada
            let f = formatar(t, Recuo::Espacos(2)).unwrap_or_else(|e| panic!("{t:?}: {e}"));
            assert_eq!(compacto(&f.replace('\u{FEFF}', "")), compacto(&t.replace('\u{FEFF}', "")), "{t:?}");
            assert_eq!(formatar(&f, Recuo::Espacos(2)).unwrap(), f, "{t:?}");
            for byte in (0..=t.len()).step_by(7) {
                let caminho = arvore.caminho_em(byte);
                assert!(caminho.first().is_none_or(|&n| n == arvore.raiz()));
            }
        }
        _ => unreachable!(),
    }
}

#[test]
fn nada_da_panico_com_texto_mutilado() {
    let mut sorteio = Sorteio(0x9E37_79B9_7F4A_7C15);
    for base in [EXEMPLO, COMPACTO, "[1,{\"a\":[true,null,\"x\\u00e7\"]},-2.5e3]"] {
        conferir_analise_de_texto_qualquer(base);
        // todo prefixo (o documento cortado em qualquer ponto)
        for (fim, _) in base.char_indices() {
            conferir_analise_de_texto_qualquer(&base[..fim]);
        }
        // e as mutações
        for _ in 0..4000 {
            let t = mutar(base, &mut sorteio);
            conferir_analise_de_texto_qualquer(&t);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Medidas (`#[ignore]`: `cargo test --release -p texto -- --ignored --nocapture`)

fn documento_de_verdade(bytes: usize) -> String {
    let registro =
        r#"{"id":12345,"nome":"Fulano de Tal","ativo":true,"saldo":1234.56,"tags":["a","b","ç"],"obs":null,"fim":{}}"#;
    let n = bytes / (registro.len() + 2);
    format!("[\n{}\n]", vec![registro; n].join(",\n"))
}

#[test]
#[ignore = "medição: cargo test --release -p texto -- --ignored --nocapture"]
fn oito_mib_em_menos_de_um_segundo() {
    const OITO_MIB: usize = 8 * 1024 * 1024;
    // JSON de verdade
    let texto = documento_de_verdade(OITO_MIB);
    let t = Instant::now();
    let a = analisar(&texto);
    let analisar_ms = t.elapsed();
    let arvore = a.arvore.expect("válido");
    let t = Instant::now();
    assert!(validar(&texto).is_none());
    let validar_ms = t.elapsed();
    let t = Instant::now();
    let f = formatar(&texto, Recuo::Espacos(2)).unwrap();
    let formatar_ms = t.elapsed();
    let t = Instant::now();
    let mut fundura = 0;
    for i in 0..1000 {
        fundura += arvore.caminho_em(i * (texto.len() / 1000)).len();
    }
    let caminho_ms = t.elapsed();
    assert!(fundura > 1000);
    eprintln!(
        "8 MiB de JSON de verdade ({} nós, {} bytes por nó): analisar {analisar_ms:?}, validar {validar_ms:?}, formatar {formatar_ms:?} ({} bytes), 1000 buscas de byte {caminho_ms:?}",
        arvore.total(),
        texto.len() / arvore.total(),
        f.len()
    );
    assert!(analisar_ms.as_secs_f64() < 1.0, "analisar levou {analisar_ms:?}");
    assert!(validar_ms.as_secs_f64() < 1.0, "validar levou {validar_ms:?}");
    assert!(formatar_ms.as_secs_f64() < 1.0, "formatar levou {formatar_ms:?}");
    drop(arvore);

    // o pior caso: 4 milhões de números de um dígito (um nó a cada 2 bytes)
    let numeros = format!("[{}1]", "1,".repeat(OITO_MIB / 2 - 1));
    let t = Instant::now();
    let a = analisar(&numeros);
    let pior_ms = t.elapsed();
    let arvore = a.arvore.expect("válido");
    eprintln!(
        "8 MiB de números ({} nós, {} MiB de arena): analisar {pior_ms:?}",
        arvore.total(),
        arvore.total() * std::mem::size_of::<No>() / (1024 * 1024)
    );
    let t = Instant::now();
    assert!(validar(&numeros).is_none());
    eprintln!("8 MiB de números: validar {:?}", t.elapsed());
    assert!(pior_ms.as_secs_f64() < 1.0, "o pior caso levou {pior_ms:?}");

    // 100.000 níveis
    let fundo = format!("{}1{}", "[".repeat(100_000), "]".repeat(100_000));
    let t = Instant::now();
    assert!(analisar(&fundo).arvore.is_some());
    eprintln!("100.000 níveis: analisar {:?}", t.elapsed());
    eprintln!("tamanho de um nó: {} bytes", std::mem::size_of::<No>());
}
