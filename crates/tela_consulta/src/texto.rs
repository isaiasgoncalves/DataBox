//! As regras de texto da Consulta SQL: nome citado para o SQL, resumo do
//! comando no Histórico, nome sugerido para exportar, inserção no cursor e o trecho selecionado. As posições
//! são em caracteres (as do `TextEdit` do egui).

/// O `\w` do JavaScript (sem a bandeira `u`): letra ASCII, algarismo ou sublinhado.
fn palavra(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Primeira linha do comando, sem espaços repetidos, para caber numa opção do Histórico (o `resumoComando`).
pub fn resumo_comando(sql: &str) -> String {
    const MAX: usize = 72;
    let linha = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    if linha.chars().count() > MAX {
        let mut curto: String = linha.chars().take(MAX - 1).collect();
        curto.push('…');
        curto
    } else {
        linha
    }
}

/// O nome entre aspas duplas a partir de `resto` (logo depois da aspa de abertura), com `""` valendo uma aspa;
/// a mesma resposta do `"((?:[^"]|"")+)"` com o retrocesso da expressão regular.
fn entre_aspas(resto: &str) -> Option<String> {
    let chars: Vec<char> = resto.chars().collect();
    let mut nome = String::new();
    // onde terminaria o nome se a aspa de fechamento fosse a primeira de um par "" (o retrocesso)
    let mut antes_do_ultimo_par: Option<String> = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c != '"' {
            nome.push(c);
            i += 1;
        } else if chars.get(i + 1) == Some(&'"') {
            if !nome.is_empty() {
                antes_do_ultimo_par = Some(nome.clone());
            }
            nome.push('"');
            i += 2;
        } else {
            return (!nome.is_empty()).then_some(nome);
        }
    }
    antes_do_ultimo_par
}

/// Até o delimitador `fim`, com pelo menos um caractere (`[nome]` e `` `nome` ``).
fn ate(resto: &str, fim: char) -> Option<String> {
    let i = resto.find(fim)?;
    (i > 0).then(|| resto[..i].to_string())
}

/// Nome sugerido para o arquivo exportado (o `nomeParaExportar`): "consulta_clientes" quando um FROM cita uma
/// tabela; senão "consulta".
pub fn nome_para_exportar(comando: &str) -> String {
    let baixo = comando.to_ascii_lowercase();
    let mut tabela = None;
    let mut inicio = 0;
    while let Some(achado) = baixo[inicio..].find("from") {
        let pos = inicio + achado;
        inicio = pos + 4;
        // `\bfrom`: o caractere de antes não é de palavra
        if comando[..pos].chars().next_back().is_some_and(palavra) {
            continue;
        }
        // `\s+`
        let depois = &comando[pos + 4..];
        let resto = depois.trim_start();
        if resto.len() == depois.len() {
            continue;
        }
        let mut chars = resto.chars();
        let achou = match chars.next() {
            Some('"') => entre_aspas(&resto[1..]),
            Some('[') => ate(&resto[1..], ']'),
            Some('`') => ate(&resto[1..], '`'),
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                let fim = resto.find(|c: char| !(palavra(c) || c == '$')).unwrap_or(resto.len());
                Some(resto[..fim].to_string())
            }
            _ => None,
        };
        if achou.is_some() {
            tabela = achou;
            break;
        }
    }
    let tabela = tabela.unwrap_or_default();
    // `[<>:"/\\|?*\u0000-\u001f]+` → "_"
    let mut limpo = String::with_capacity(tabela.len());
    let mut em_troca = false;
    for c in tabela.chars() {
        if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || (c as u32) < 0x20 {
            if !em_troca {
                limpo.push('_');
            }
            em_troca = true;
        } else {
            limpo.push(c);
            em_troca = false;
        }
    }
    let limpo = limpo.trim();
    if limpo.is_empty() { "consulta".to_string() } else { format!("consulta_{limpo}") }
}

/// Insere `nome` (já citado) no lugar da seleção `inicio..fim`, com um espaço antes quando o texto anterior
/// termina numa palavra, aspas, colchete ou crase (o `inserirNoCursor`). Devolve o texto novo e a posição do
/// cursor, logo depois do nome.
pub fn inserir_no_cursor(texto: &str, inicio: usize, fim: usize, nome: &str) -> (String, usize) {
    let chars: Vec<char> = texto.chars().collect();
    let s = inicio.min(fim).min(chars.len());
    let e = inicio.max(fim).min(chars.len());
    let espaco = chars[..s].last().is_some_and(|&c| palavra(c) || matches!(c, '"' | ']' | '`'));
    let mut novo: String = chars[..s].iter().collect();
    if espaco {
        novo.push(' ');
    }
    novo.push_str(nome);
    let cursor = s + usize::from(espaco) + nome.chars().count();
    novo.extend(&chars[e..]);
    (novo, cursor)
}

/// O trecho selecionado (`inicio..fim` em caracteres, em qualquer ordem).
pub fn trecho(texto: &str, inicio: usize, fim: usize) -> String {
    let (s, e) = (inicio.min(fim), inicio.max(fim));
    texto.chars().skip(s).take(e - s).collect()
}

/// Um caractere sem acento e em minúscula (um por um, para os índices do texto continuarem valendo: a marca
/// combinante solta fica, em vez de sumir como no `interface::formato::sem_acento`).
fn normalizado(c: char) -> char {
    let sem = interface::formato::letra_sem_acento(c).unwrap_or(c);
    sem.to_lowercase().next().unwrap_or(sem)
}

/// A primeira ocorrência de `termo` em `texto` a partir do caractere `desde`, sem caixa nem acento, com volta ao
/// começo (a busca da barra na Consulta SQL). Devolve o trecho em caracteres (`inicio..fim`); None com o termo vazio
/// ou sem ocorrência.
pub fn achar_sem_acento(texto: &str, termo: &str, desde: usize) -> Option<(usize, usize)> {
    let t: Vec<char> = texto.chars().map(normalizado).collect();
    let p: Vec<char> = termo.chars().map(normalizado).collect();
    if p.is_empty() || p.len() > t.len() {
        return None;
    }
    let ultimo = t.len() - p.len();
    let desde = desde.min(ultimo + 1);
    (desde..=ultimo).chain(0..desde).find(|&i| t[i..i + p.len()] == p[..]).map(|i| (i, i + p.len()))
}

/// A linha e a coluna (do 1) do caractere `cursor` (a faixa da Consulta: "Linha 9, coluna 25").
pub(crate) fn linha_e_coluna(texto: &str, cursor: usize) -> (usize, usize) {
    let (mut linha, mut coluna) = (1, 1);
    for c in texto.chars().take(cursor) {
        if c == '\n' {
            linha += 1;
            coluna = 1;
        } else {
            coluna += 1;
        }
    }
    (linha, coluna)
}

/// O trecho (em caracteres, a partir de `base`) de cada comando no texto executado, na ordem: cada um é procurado
/// depois do anterior, como o `dividir_comandos` os tirou (sem os espaços das pontas). None quando um não
/// aparece (daí em diante, nenhum).
pub(crate) fn posicoes_dos_comandos<'a>(
    texto: &str,
    base: usize,
    comandos: impl IntoIterator<Item = &'a str>,
) -> Vec<Option<std::ops::Range<usize>>> {
    let mut posicoes = Vec::new();
    // onde a busca continua, em bytes e em caracteres
    let (mut desde, mut chars) = (0usize, 0usize);
    let mut perdido = false;
    for comando in comandos {
        let achado = if perdido { None } else { texto.get(desde..).and_then(|resto| resto.find(comando)) };
        match achado {
            Some(rel) => {
                let inicio = chars + texto[desde..desde + rel].chars().count();
                let n = comando.chars().count();
                posicoes.push(Some(base + inicio..base + inicio + n));
                desde += rel + comando.len();
                chars = inicio + n;
            }
            None => {
                perdido = true;
                posicoes.push(None);
            }
        }
    }
    posicoes
}

/// O nome que a frase de erro cita depois dos dois pontos ("A tabela não existe: X." → "X"), quando é uma palavra
/// só (o melhor esforço do sublinhado no editor).
pub(crate) fn nome_citado(mensagem: &str) -> Option<&str> {
    let (_, depois) = mensagem.rsplit_once(": ")?;
    let nome = depois.trim().trim_end_matches('.').trim();
    (!nome.is_empty() && !nome.contains(char::is_whitespace)).then_some(nome)
}

/// O trecho (em caracteres, dentro de `comando`) da primeira vez que `nome` aparece nele.
pub(crate) fn trecho_do_nome(comando: &str, nome: &str) -> Option<std::ops::Range<usize>> {
    let b = comando.find(nome)?;
    let inicio = comando[..b].chars().count();
    Some(inicio..inicio + nome.chars().count())
}

/// Em que linha (do 0) fica o caractere `i`.
pub(crate) fn linha_do_caractere(texto: &str, i: usize) -> usize {
    texto.chars().take(i).filter(|&c| c == '\n').count()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn posicao_e_linhas_dos_comandos() {
        let sql = "SELECT 1;\nUPDATE t SET a = 1;\n  SELECT * FROM tabela_que_nao_existe;";
        assert_eq!(linha_e_coluna(sql, 0), (1, 1));
        assert_eq!(linha_e_coluna(sql, 10), (2, 1));
        assert_eq!(linha_e_coluna(sql, sql.chars().count()), (3, 39));
        let comandos = ["SELECT 1;", "UPDATE t SET a = 1;", "SELECT * FROM tabela_que_nao_existe;"];
        let p = posicoes_dos_comandos(sql, 0, comandos);
        assert_eq!(p, [Some(0..9), Some(10..29), Some(32..68)]);
        assert_eq!(linha_do_caractere(sql, 32), 2);
        // o mesmo comando duas vezes: cada um na sua vez; com o trecho de uma seleção, a partir da base
        assert_eq!(
            posicoes_dos_comandos("SELECT 1; SELECT 1;", 5, ["SELECT 1;", "SELECT 1;"]),
            [Some(5..14), Some(15..24)]
        );
        assert_eq!(posicoes_dos_comandos("ação; x", 0, ["ação;", "y", "x"]), [Some(0..5), None, None]);
        assert_eq!(nome_citado("A tabela não existe: tabela_que_nao_existe."), Some("tabela_que_nao_existe"));
        assert_eq!(nome_citado("Erro de sintaxe: near \"SELEC\": syntax error"), None);
        assert_eq!(nome_citado("A consulta foi interrompida."), None);
        assert_eq!(trecho_do_nome("SELECT * FROM ação_x;", "ação_x"), Some(14..20));
    }

    #[test]
    fn achar_sem_caixa_nem_acento_e_com_volta() {
        let sql = "SELECT nome FROM clientes WHERE cidade = 'São Paulo';\nselect * from Clientes;";
        // a partir do começo, a primeira; do fim da primeira, a seguinte; do fim da última, volta ao começo
        assert_eq!(achar_sem_acento(sql, "clientes", 0), Some((17, 25)));
        assert_eq!(achar_sem_acento(sql, "CLIENTES", 25), Some((68, 76)));
        assert_eq!(achar_sem_acento(sql, "clientes", 76), Some((17, 25)));
        assert_eq!(achar_sem_acento(sql, "sao paulo", 0), Some((42, 51)));
        assert_eq!(achar_sem_acento(sql, "", 0), None);
        assert_eq!(achar_sem_acento(sql, "pedidos", 0), None);
        assert_eq!(achar_sem_acento("ab", "abc", 0), None);
    }

    #[test]
    fn palavras_reservadas_em_ordem() {
        // a lista do interface (a mesma do realce do editor), com as 147 do SQLite
        use interface::codigo::{PALAVRAS_RESERVADAS, palavra_reservada as reservada};
        let mut ordenadas = PALAVRAS_RESERVADAS;
        ordenadas.sort_unstable();
        assert_eq!(ordenadas, PALAVRAS_RESERVADAS, "a busca binária precisa da lista em ordem");
        assert_eq!(PALAVRAS_RESERVADAS.len(), 147);
        assert!(PALAVRAS_RESERVADAS.iter().all(|p| reservada(p) && reservada(&p.to_lowercase())));
    }

    #[test]
    fn resumo_numa_linha_e_cortado() {
        assert_eq!(resumo_comando("  SELECT *\n  FROM\tclientes  "), "SELECT * FROM clientes");
        let longo = format!("SELECT {}", "x".repeat(100));
        let r = resumo_comando(&longo);
        assert_eq!(r.chars().count(), 72);
        assert!(r.ends_with('…'));
    }

    #[test]
    fn nome_para_exportar_como_la() {
        assert_eq!(nome_para_exportar("SELECT id, nome FROM clientes WHERE ativo = 1"), "consulta_clientes");
        assert_eq!(nome_para_exportar("select * from \"Tabela com espaço\""), "consulta_Tabela com espaço");
        assert_eq!(nome_para_exportar("SELECT * FROM [itens pedido]"), "consulta_itens pedido");
        assert_eq!(nome_para_exportar("SELECT * FROM `notas`"), "consulta_notas");
        assert_eq!(nome_para_exportar("SELECT 1"), "consulta");
        assert_eq!(nome_para_exportar(""), "consulta");
        // "from" dentro de uma palavra não conta; o primeiro FROM que cita uma tabela vale
        assert_eq!(nome_para_exportar("SELECT fromage FROM (SELECT 1) JOIN t2"), "consulta");
        assert_eq!(nome_para_exportar("SELECT x FROM (SELECT * FROM pedidos)"), "consulta_pedidos");
        assert_eq!(nome_para_exportar("SELECT * FROM a$b"), "consulta_a$b");
        // aspas dobradas viram uma; caracteres proibidos em nome de arquivo viram "_"
        assert_eq!(nome_para_exportar("SELECT * FROM \"a\"\"b\""), "consulta_a_b");
        assert_eq!(nome_para_exportar("SELECT * FROM \"x:/?y\""), "consulta_x_y");
        // sem a aspa de fechamento, o retrocesso acha o nome antes do último par
        assert_eq!(nome_para_exportar("SELECT * FROM \"ab\"\" x"), "consulta_ab");
        assert_eq!(nome_para_exportar("SELECT * FROM \"ab"), "consulta");
    }

    #[test]
    fn insere_com_espaco_depois_de_palavra() {
        assert_eq!(inserir_no_cursor("SELECT * FROM", 13, 13, "clientes"), ("SELECT * FROM clientes".into(), 22));
        assert_eq!(inserir_no_cursor("SELECT * FROM ", 14, 14, "clientes"), ("SELECT * FROM clientes".into(), 22));
        assert_eq!(inserir_no_cursor("", 0, 0, "\"T x\""), ("\"T x\"".into(), 5));
        // a seleção é trocada pelo nome
        assert_eq!(inserir_no_cursor("FROM xyz;", 5, 8, "notas"), ("FROM notas;".into(), 10));
        // posições em caracteres, com acento antes
        assert_eq!(inserir_no_cursor("ação", 4, 4, "t"), ("ação t".into(), 6));
        assert_eq!(inserir_no_cursor("(", 1, 1, "t"), ("(t".into(), 2));
    }

    #[test]
    fn trecho_em_caracteres() {
        assert_eq!(trecho("SELECT 1; SELECT 2", 10, 18), "SELECT 2");
        assert_eq!(trecho("ação", 3, 1), "çã");
        assert_eq!(trecho("abc", 2, 9), "c");
    }
}
