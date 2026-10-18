//! A exportação do banco de exemplo (fica aqui porque depende dele): as linhas vêm do `Servico::exportar` (o `Banco::iterar_*` + `Escritor`), conferidas contra o arquivo lido pelo rusqlite.

use comum::{Valor, formatar_real};
use exportar::{Formato, Opcoes};
use rusqlite::Connection;

use super::leitura::{Json, base64, ler_csv, ler_csv_texto, ler_json, ler_xlsx, ler_zip, letra_coluna, sem_bom};
use super::*;
use crate::OrigemExportacao;

/// As tabelas do banco: tabelas e virtuais do main, fora as sqlite_*, por nome.
fn tabelas(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT name FROM pragma_table_list WHERE schema = 'main' AND type IN ('table', 'virtual') \
             AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' ORDER BY name",
        )
        .unwrap();
    stmt.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
}

/// (colunas, linhas) de uma tabela.
type Conteudo = (Vec<String>, Vec<Vec<Valor>>);

/// (colunas, linhas) de `SELECT * FROM <nome>`.
fn conteudo(conn: &Connection, nome: &str) -> Conteudo {
    let mut stmt = conn.prepare(&format!("SELECT * FROM {}", comum::q(nome))).unwrap();
    let colunas: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
    let n = colunas.len();
    let mut linhas = Vec::new();
    let mut r = stmt.query([]).unwrap();
    while let Some(l) = r.next().unwrap() {
        linhas.push((0..n).map(|i| Valor::de_ref(l.get_ref(i).unwrap())).collect());
    }
    (colunas, linhas)
}

/// O valor como o JSON exportado o traz (BLOB em base64, inteiros exatos).
fn confere_json(lido: &Json, original: &Valor, onde: &str) {
    match (original, lido) {
        (Valor::Nulo, Json::Nulo) => {}
        (Valor::Inteiro(i), Json::Numero(n)) => assert_eq!(n, &i.to_string(), "{onde}"),
        (Valor::Real(r), Json::Numero(n)) => assert_eq!(n.parse::<f64>().unwrap(), *r, "{onde}"),
        (Valor::Texto(t), Json::Texto(s)) => assert_eq!(s, t, "{onde}"),
        (Valor::Blob(b), Json::Texto(s)) => assert_eq!(s, &base64(b), "{onde}"),
        (o, l) => panic!("{onde}: {o:?} saiu como {l:?}"),
    }
}

#[test]
fn banco_de_exemplo_nos_tres_formatos() {
    let a = ambiente("exemplo-tres-formatos");
    let conn = Connection::open(&a.exemplo).unwrap();
    let nomes = tabelas(&conn);
    let esperado: Vec<(String, Conteudo)> = nomes.iter().map(|n| (n.clone(), conteudo(&conn, n))).collect();
    drop(conn);
    let de = |nome: &str| &esperado.iter().find(|(n, _)| n == nome).unwrap().1;
    assert_eq!(de("clientes").1.len() as u64, N_CLIENTES);
    assert_eq!(de("produtos").1.len() as u64, N_PRODUTOS);
    assert_eq!(de("pedidos").1.len() as u64, N_PEDIDOS);
    assert_eq!(de("eventos").1.len() as u64, N_EVENTOS);
    assert_eq!(de("itens_pedido").1.len(), 456);
    let total: usize = esperado.iter().map(|(_, (_, l))| l.len()).sum();

    let mut resumos = std::collections::HashMap::new();
    for formato in Formato::TODOS {
        let destino = a.join(&format!("exemplo.{}", formato.como_str()));
        let origem = OrigemExportacao::Banco { tabelas: Some(nomes.clone()) };
        let r = a.exportar(origem, formato, &destino, None).unwrap();
        assert_eq!((r.tabelas, r.linhas), (nomes.len(), total as u64), "{formato}");
        resumos.insert(formato, r);
    }

    // JSON: tudo idêntico ao banco, com BLOB em base64 e inteiros grandes exatos
    let dados = ler_json(&resumos[&Formato::Json].caminho);
    assert_eq!(dados.campo("banco").texto(), "exemplo.db");
    let tabelas_json = dados.campo("tabelas");
    assert_eq!(tabelas_json.chaves(), nomes);
    for (nome, (colunas, linhas)) in &esperado {
        let lidas = tabelas_json.campo(nome).lista();
        assert_eq!(lidas.len(), linhas.len(), "{nome}");
        for (lida, linha) in lidas.iter().zip(linhas) {
            assert_eq!(&lida.chaves(), colunas, "{nome}");
            for (coluna, valor) in colunas.iter().zip(linha) {
                confere_json(lida.campo(coluna), valor, &format!("{nome}.{coluna}"));
            }
        }
    }
    let series: Vec<String> = tabelas_json
        .campo("produtos")
        .lista()
        .iter()
        .map(|p| match p.campo("serie") {
            Json::Numero(n) => n.clone(),
            outro => panic!("{outro:?}"),
        })
        .collect();
    assert!(series.contains(&"9007199254740997".to_string())); // P004: 2^53 + 1 + 4

    // Excel: uma planilha por tabela, contagens, texto para inteiros grandes e BLOB em base64
    let planilhas = ler_xlsx(&resumos[&Formato::Xlsx].caminho);
    assert_eq!(planilhas.iter().map(|p| p.nome.clone()).collect::<Vec<_>>(), nomes);
    for (folha, (nome, (colunas, linhas))) in planilhas.iter().zip(&esperado) {
        let ultima = letra_coluna(colunas.len());
        assert_eq!(folha.filtro(), Some(format!("A1:{ultima}{}", linhas.len() + 1)), "{nome}");
        let cabecalho: Vec<String> =
            (1..=colunas.len()).map(|c| folha.texto(&format!("{}1", letra_coluna(c))).unwrap()).collect();
        assert_eq!(&cabecalho, colunas, "{nome}");
        // contado no XML: a última linha de "Tabela com espaço" é toda NULL
        assert_eq!(folha.linhas(), linhas.len() + 1, "{nome}");
    }
    let produtos = &planilhas[nomes.iter().position(|n| n == "produtos").unwrap()];
    let (colunas_prod, linhas_prod) = de("produtos");
    let coluna_de = |nome: &str| letra_coluna(colunas_prod.iter().position(|c| c == nome).unwrap() + 1);
    for (i, linha) in linhas_prod.iter().enumerate() {
        let r = i + 2;
        let valor = |nome: &str| &linha[colunas_prod.iter().position(|c| c == nome).unwrap()];
        let Valor::Texto(codigo) = valor("codigo") else { panic!("codigo") };
        assert_eq!(produtos.texto(&format!("{}{r}", coluna_de("codigo"))).as_deref(), Some(codigo.as_str()));
        let Valor::Inteiro(serie) = valor("serie") else { panic!("serie") };
        let celula_serie = format!("{}{r}", coluna_de("serie"));
        if *serie > 10i64.pow(15) {
            assert_eq!(produtos.texto(&celula_serie), Some(serie.to_string()));
        } else {
            assert_eq!(produtos.numero(&celula_serie), Some(serie.to_string()));
        }
        let celula_foto = format!("{}{r}", coluna_de("foto"));
        match valor("foto") {
            Valor::Blob(b) if !b.is_empty() => assert_eq!(produtos.texto(&celula_foto), Some(base64(b))),
            _ => assert!(produtos.celulas.get(&celula_foto).is_none_or(|(_, v)| v.is_empty()), "{celula_foto}"),
        }
    }
    let espaco = &planilhas[nomes.iter().position(|n| n == "Tabela com espaço").unwrap()];
    assert_eq!(espaco.texto("A1").as_deref(), Some("coluna \"citada\""));
    let clientes = &planilhas[nomes.iter().position(|n| n == "clientes").unwrap()];
    let Valor::Texto(primeiro_nome) = &de("clientes").1[0][1] else { panic!("nome") };
    assert_eq!(clientes.texto("B2").as_ref(), Some(primeiro_nome));

    // CSV: um arquivo por tabela dentro do zip, com as mesmas contagens
    let pacote = ler_zip(&resumos[&Formato::Csv].caminho);
    assert_eq!(
        pacote.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        nomes.iter().map(|n| format!("{n}.csv")).collect::<Vec<_>>()
    );
    for ((parte, bytes), (nome, (colunas, linhas))) in pacote.iter().zip(&esperado) {
        let lidas = ler_csv_texto(&sem_bom(bytes), ';');
        assert_eq!(&lidas[0], colunas, "{parte}");
        assert_eq!(lidas.len(), linhas.len() + 1, "{nome}");
    }
    let (_, pedidos) = pacote.iter().find(|(n, _)| n == "pedidos.csv").unwrap();
    let pedidos = ler_csv_texto(&sem_bom(pedidos), ';');
    let obs = |id: &str| pedidos[1..].iter().find(|l| l[0] == id).unwrap()[4].clone();
    assert_eq!(obs("10"), format!("Texto longo {}", "x".repeat(700)));
    assert_eq!(obs("1"), "");
}

#[test]
fn tabela_e_consulta_do_banco_de_exemplo() {
    let a = ambiente("exemplo-tabela-consulta");
    let destino = a.join("eventos.csv");
    let tabela = OrigemExportacao::Tabela { nome: "eventos".into(), filtro: Default::default() };
    let opcoes = Opcoes { csv_decimal: ',', ..Opcoes::default() };
    let resumo = a.exportar_com(tabela, Formato::Csv, &destino, opcoes, false, None).unwrap();
    let eventos = cru(&a.exemplo, "SELECT * FROM eventos");

    let consulta = OrigemExportacao::Consulta { sql: "SELECT id, nome, id FROM clientes ORDER BY id LIMIT 3".into() };
    let r_consulta = a.exportar(consulta, Formato::Json, &a.join("consulta.json"), None).unwrap();

    let produtos =
        OrigemExportacao::Consulta { sql: "SELECT codigo, serie, foto FROM produtos ORDER BY codigo".into() };
    let planilha = a.exportar(produtos, Formato::Xlsx, &a.join("produtos.xlsx"), None).unwrap();

    assert_eq!(resumo.linhas, N_EVENTOS);
    let lidas = ler_csv(&destino, ';');
    assert_eq!(lidas[0], ["momento", "tipo", "valor", "dados"]);
    assert_eq!(lidas.len() - 1, eventos.len());
    for (original, lida) in eventos.iter().zip(&lidas[1..]) {
        let valor = match &original[2] {
            Valor::Nulo => String::new(),
            Valor::Real(r) => formatar_real(*r).replace('.', ","),
            outro => outro.texto_exibicao(),
        };
        assert_eq!(lida[2], valor);
        assert_eq!(lida[3], original[3].texto_exibicao());
    }

    assert_eq!(r_consulta.linhas, 3);
    let objetos = ler_json(&a.join("consulta.json"));
    for o in objetos.lista() {
        assert_eq!(o.chaves(), ["id", "nome", "id_2"]);
        assert_eq!(o.campo("id"), o.campo("id_2"));
    }
    assert_eq!(objetos.lista().len(), 3);

    assert_eq!(planilha.linhas, N_PRODUTOS);
    // a planilha de uma consulta se chama "consulta" (lá o teste passava nome="produtos")
    let folha = &ler_xlsx(&a.join("produtos.xlsx"))[0];
    assert_eq!(folha.nome, "consulta");
    let linha_de =
        |codigo: &str| (2..=N_PRODUTOS + 1).find(|r| folha.texto(&format!("A{r}")).as_deref() == Some(codigo)).unwrap();
    assert_eq!(folha.texto(&format!("B{}", linha_de("P004"))).as_deref(), Some("9007199254740997"));
    assert_eq!(folha.numero(&format!("B{}", linha_de("P001"))).as_deref(), Some("1"));
}
