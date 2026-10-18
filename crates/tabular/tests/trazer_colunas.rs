//! O "Trazer colunas" de ponta a ponta, sem tela: ler as duas tabelas, sugerir as chaves, casar, trazer colunas
//! novas, atualizar uma que existe, marcar as linhas sem par, desfazer e salvar.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use comum::Resultado;
use tabular::colunas::{
    ChavesRepetidas, ModoAtualizacao, OpcoesCasamento, OrigemDoTexto, QuandoSobram, atualizar_coluna, casar,
    colunas_coladas, escrever_coluna, ler_tsv, nome_da_coluna_trazida, posicionar, rotulo_da_origem, sugerir_chaves,
};
use tabular::csv::{Leitor, OpcoesAbertura, Tabela};

struct Pasta(PathBuf);

impl Pasta {
    fn nova(nome: &str) -> Pasta {
        static N: AtomicUsize = AtomicUsize::new(0);
        let c = std::env::temp_dir().join(format!(
            "tabular-trazer-{nome}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&c);
        fs::create_dir_all(&c).unwrap();
        Pasta(c)
    }

    fn arquivo(&self, nome: &str, texto: &str) -> PathBuf {
        let c = self.0.join(nome);
        fs::write(&c, texto).unwrap();
        c
    }
}

impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn sem_progresso(_: u64, _: u64) -> Resultado<()> {
    Ok(())
}

const ESTOQUE: &str = concat!(
    "codigo;produto;deposito;estoque;regiao\r\n",
    "004718;Caderno universitário;Guarulhos;412;Sudeste\r\n",
    "004725;Caneta gel;Guarulhos;1.380;\r\n",
    "004731;Mochila escolar;Contagem;96;Sudeste\r\n",
    "019044;Agenda 2025;Contagem;18;Norte\r\n",
    "005102;Estojo duplo;Guarulhos;233;\r\n",
    " 005117 ;Marca-texto;Recife;2.045;Nordeste\r\n",
);

const CATALOGO: &str = concat!(
    "cod_produto;descricao;preco;categoria;area\r\n",
    "004718;Caderno univ.;24,90;Cadernos;Sul\r\n",
    "004725;Caneta gel;4,50;Escrita;Sul\r\n",
    "004731;Mochila;129,90;Mochilas;Sudeste\r\n",
    "005102;Estojo;27,40;Estojos;Centro-Oeste\r\n",
    "005117;Marca-texto;6,90;Escrita;Nordeste\r\n",
    "004725;Caneta gel (2ª);5,00;Escrita;\r\n",
);
#[test]
fn trazer_colunas_de_um_csv_para_outro() {
    let pasta = Pasta::nova("e2e");
    let destino_caminho = pasta.arquivo("estoque_2026.csv", ESTOQUE);
    let origem_caminho = pasta.arquivo("catalogo_2025.csv", CATALOGO);
    let mut destino = Tabela::abrir(&destino_caminho, &OpcoesAbertura::default()).unwrap();
    let mut origem = Leitor::abrir(&origem_caminho, &OpcoesAbertura::default()).unwrap();
    assert_eq!((destino.linhas(), origem.linhas()), (6, 6));
    let opcoes = OpcoesCasamento::default();

    // 1. a sugestão automática das chaves (os nomes diferem; vale o par com mais valores em comum)
    let amostra_destino = destino.colunas_de(&[0, 1, 2, 3, 4], &mut sem_progresso).unwrap();
    let amostra_origem = origem.colunas_de_sem_progresso(&[0, 1, 2, 3, 4]).unwrap();
    let (kd, ko) =
        sugerir_chaves(&destino.nomes(), &amostra_destino, origem.nomes(), &amostra_origem, &opcoes).unwrap();
    assert_eq!((kd, ko), (0, 0), "codigo = cod_produto");

    // 2. o casamento
    let c = casar(&amostra_destino[kd], &amostra_origem[ko], &opcoes);
    assert_eq!((c.com_par(), c.total(), c.percentual()), (5, 6, 83));
    assert_eq!(c.sem_par, vec![3], "019044 não está no catálogo");
    assert_eq!(c.repetidas.len(), 1);
    assert_eq!(c.repetidas[0].chave, "004725");
    assert_eq!(c.repetidas[0].linhas, vec![1, 5]);

    // 3. trazer preço e categoria como colunas novas (um passo só)
    let rotulo = rotulo_da_origem("catalogo_2025.csv");
    assert_eq!(rotulo, "catalogo");
    let preco = c.trazer(&amostra_origem[2], ChavesRepetidas::Primeira);
    let categoria = c.trazer(&amostra_origem[3], ChavesRepetidas::TodasJuntas);
    destino
        .em_um_passo(|t| {
            let nomes = t.nomes();
            let nome_preco = nome_da_coluna_trazida("preco", &rotulo, &nomes);
            let valores: Vec<String> = preco.iter().map(|v| v.clone().unwrap_or_default()).collect();
            t.inserir_coluna_com_valores(t.colunas(), &nome_preco, &valores)?;
            let nomes = t.nomes();
            let nome_cat = nome_da_coluna_trazida("categoria", &rotulo, &nomes);
            let valores: Vec<String> = categoria.iter().map(|v| v.clone().unwrap_or_default()).collect();
            t.inserir_coluna_com_valores(t.colunas(), &nome_cat, &valores)?;
            // 4. atualizar a regiao só onde está vazia
            let trazidos = c.trazer(&amostra_origem[4], ChavesRepetidas::Primeira);
            let atual = &amostra_destino[4];
            let a = atualizar_coluna(atual, &trazidos, ModoAtualizacao::SoCelulasVazias);
            assert_eq!(a.alteradas, vec![1, 4], "as duas células vazias que têm par com valor");
            let novos: Vec<Option<String>> = a.valores.iter().map(|v| Some(v.clone())).collect();
            t.preencher_coluna(4, &novos)
        })
        .unwrap();
    assert_eq!(destino.nomes(), ["codigo", "produto", "deposito", "estoque", "regiao", "preco", "categoria"]);
    let pagina = destino.pagina(0, 10).unwrap();
    assert_eq!(pagina[0], ["004718", "Caderno universitário", "Guarulhos", "412", "Sudeste", "24,90", "Cadernos"]);
    assert_eq!(pagina[1], ["004725", "Caneta gel", "Guarulhos", "1.380", "Sul", "4,50", "Escrita; Escrita"]);
    assert_eq!(pagina[3], ["019044", "Agenda 2025", "Contagem", "18", "Norte", "", ""], "sem par: vazio");
    assert_eq!(pagina[4][4], "Centro-Oeste");
    assert_eq!(pagina[5][0], " 005117 ", "a chave com espaços casou, o texto continua como estava");
    assert_eq!(pagina[5][5], "6,90");

    // uma desfeita leva as três alterações (as duas colunas e a atualização)
    assert!(destino.desfazer());
    assert_eq!(destino.colunas(), 5);
    assert!(!destino.alterada());
    assert!(destino.refazer());
    assert_eq!(destino.colunas(), 7);

    // 5. salvar e reabrir
    destino.salvar(&mut sem_progresso).unwrap();
    let mut de_novo = Tabela::abrir(&destino_caminho, &OpcoesAbertura::default()).unwrap();
    assert_eq!(de_novo.pagina(0, 10).unwrap(), pagina);
    let texto = fs::read_to_string(&destino_caminho).unwrap();
    assert!(texto.starts_with("codigo;produto;deposito;estoque;regiao;preco;categoria\r\n"), "{texto}");
    assert!(texto.contains("019044;Agenda 2025;Contagem;18;Norte;;\r\n"));
}

#[test]
fn trazer_com_opcoes_diferentes_e_chaves_repetidas() {
    let origem = vec!["A1".to_string(), "a1 ".to_string(), "007".to_string(), "B2".to_string()];
    let valores = vec!["x".to_string(), "y".to_string(), "z".to_string(), "w".to_string()];
    let destino = vec!["a1".to_string(), "7".to_string(), "b2".to_string()];
    let c = casar(&destino, &origem, &OpcoesCasamento::default());
    assert_eq!(c.sem_par, vec![1], "007 e 7 só casam com Números pelo valor");
    assert_eq!(c.trazer(&valores, ChavesRepetidas::Ultima), [Some("y".to_string()), None, Some("w".to_string())]);
    assert_eq!(c.trazer(&valores, ChavesRepetidas::TodasJuntas)[0].as_deref(), Some("x; y"));
    let por_valor = OpcoesCasamento { numeros_pelo_valor: true, ..OpcoesCasamento::default() };
    let c = casar(&destino, &origem, &por_valor);
    assert!(c.sem_par.is_empty());
    assert_eq!(c.trazer(&valores, ChavesRepetidas::Primeira)[1].as_deref(), Some("z"));
}

#[test]
fn copiar_uma_coluna_e_colar_em_outra_tabela() {
    let pasta = Pasta::nova("tsv");
    let a = pasta.arquivo("a.csv", "codigo;valor\r\n1;\"12,90\"\r\n2;\r\n3;\"com\ttab e \"\"aspas\"\"\"\r\n");
    let mut origem = Tabela::abrir(&a, &OpcoesAbertura::default()).unwrap();
    let valores = origem.colunas_de(&[1], &mut sem_progresso).unwrap().remove(0);
    // Copiar coluna: TSV com o nome na primeira linha
    let area = escrever_coluna(Some("valor"), &valores);
    let linhas = ler_tsv(&area);
    let coladas = colunas_coladas(&linhas, OrigemDoTexto::Databox, ',');
    assert_eq!(coladas.len(), 1);
    assert_eq!(coladas[0].nome.as_deref(), Some("valor"));
    assert_eq!(coladas[0].valores, valores);

    // colar em outra tabela com 2 linhas: sobra 1 valor → cortar ou acrescentar
    let b = pasta.arquivo("b.csv", "id\r\nx\r\ny\r\n");
    let mut destino = Tabela::abrir(&b, &OpcoesAbertura::default()).unwrap();
    let p = posicionar(&coladas[0].valores, usize::try_from(destino.linhas()).unwrap(), QuandoSobram::Acrescentar);
    assert_eq!((p.linhas_a_acrescentar, p.linhas_vazias), (1, 0));
    destino
        .em_um_passo(|t| {
            t.acrescentar_linhas(p.linhas_a_acrescentar as u64)?;
            t.inserir_coluna_com_valores(1, "valor", &p.valores)
        })
        .unwrap();
    assert_eq!(destino.linhas(), 3);
    assert_eq!(destino.celula(2, 1).unwrap(), "com\ttab e \"aspas\"");
    destino.salvar(&mut sem_progresso).unwrap();
    let texto = fs::read_to_string(&b).unwrap();
    assert_eq!(texto, "id;valor\r\nx;12,90\r\ny;\r\n;\"com\ttab e \"\"aspas\"\"\"\r\n");
    assert!(!destino.desfazer());
}

#[test]
fn colar_texto_do_excel_com_cabecalho_detectado() {
    let do_excel = "preco\r\n12,90\r\n4,50\r\n";
    let c = colunas_coladas(&ler_tsv(do_excel), OrigemDoTexto::Externo, ',');
    assert_eq!(c[0].nome.as_deref(), Some("preco"));
    assert_eq!(c[0].valores, ["12,90", "4,50"]);
    let sem_nome = "Sul\r\nNorte\r\nSul\r\n";
    let c = colunas_coladas(&ler_tsv(sem_nome), OrigemDoTexto::Externo, ',');
    assert_eq!(c[0].nome, None);
    assert_eq!(c[0].valores, ["Sul", "Norte", "Sul"]);
}
