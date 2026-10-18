//! A leitura e a gravação do CSV fecham o círculo com o que o Exportar do DataBox faz: o que ele grava abre
//! igual, e o que o `tabular` regrava de uma linha sai com os mesmos bytes do Exportar.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use comum::{Resultado, Valor};
use exportar::{Acompanhamento, Formato as FormatoDeSaida, Opcoes, exportar_linhas};
use tabular::csv::{Codificacao, EstiloAspas, FimDeLinha, OpcoesAbertura, Tabela};

struct Pasta(PathBuf);

impl Pasta {
    fn nova(nome: &str) -> Pasta {
        static N: AtomicUsize = AtomicUsize::new(0);
        let c = std::env::temp_dir().join(format!(
            "tabular-exportar-{nome}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&c);
        fs::create_dir_all(&c).unwrap();
        Pasta(c)
    }

    fn join(&self, nome: &str) -> PathBuf {
        self.0.join(nome)
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

fn colunas() -> Vec<String> {
    ["id", "nome", "preco", "obs"].iter().map(|s| s.to_string()).collect()
}

fn texto(s: &str) -> Valor {
    Valor::Texto(s.to_string())
}

fn linhas() -> Vec<Resultado<Vec<Valor>>> {
    vec![
        Ok(vec![Valor::Inteiro(1), texto("Caderno; 96 fls"), Valor::Real(24.9), texto("linha 1\nlinha 2")]),
        Ok(vec![Valor::Inteiro(2), texto("com \"aspas\""), Valor::Real(4.5), Valor::Nulo]),
        Ok(vec![Valor::Inteiro(3), texto("=1+1"), Valor::Real(1_234_567.5), texto("")]),
        Ok(vec![Valor::Inteiro(4), texto("ação, São Paulo — Ñandú"), Valor::Real(-0.25), texto("  espaços  ")]),
        Ok(vec![Valor::Inteiro(5), texto("004718"), Valor::Inteiro(7), texto("fim")]),
    ]
}

fn exportar(pasta: &Pasta, nome: &str, opcoes: &Opcoes) -> PathBuf {
    let destino = pasta.join(nome);
    exportar_linhas(&colunas(), linhas(), FormatoDeSaida::Csv, &destino, opcoes, "tabela", Acompanhamento::default())
        .unwrap();
    destino
}

fn tudo(t: &mut Tabela) -> Vec<Vec<String>> {
    t.pagina(0, usize::try_from(t.linhas()).unwrap() + 1).unwrap()
}

fn esperado_com_virgula() -> Vec<Vec<String>> {
    let l = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    vec![
        l(&["1", "Caderno; 96 fls", "24,9", "linha 1\nlinha 2"]),
        l(&["2", "com \"aspas\"", "4,5", ""]),
        l(&["3", "'=1+1", "1234567,5", ""]), // a proteção de fórmula do Exportar fica no texto, como está no arquivo
        l(&["4", "ação, São Paulo — Ñandú", "-0,25", "  espaços  "]),
        l(&["5", "004718", "7", "fim"]),
    ]
}

#[test]
fn abre_o_que_o_exportar_grava() {
    let pasta = Pasta::nova("abre");
    let caminho = exportar(&pasta, "padrao.csv", &Opcoes::default());
    let mut t = Tabela::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
    let f = t.formato();
    assert_eq!(f.codificacao, Codificacao::Utf8ComBom);
    assert_eq!((f.separador, f.decimal, f.fim_de_linha, f.aspas), (';', ',', FimDeLinha::Crlf, EstiloAspas::Minimas));
    assert!(f.cabecalho && f.termina_em_quebra);
    assert_eq!(t.nomes(), ["id", "nome", "preco", "obs"]);
    assert_eq!(t.linhas(), 5);
    assert_eq!(tudo(&mut t), esperado_com_virgula());
}

#[test]
fn abre_com_cada_separador_e_decimal_do_exportar() {
    let pasta = Pasta::nova("opcoes");
    for (sep, decimal) in [(';', ','), (';', '.'), (',', '.'), ('\t', ','), ('\t', '.')] {
        let opcoes = Opcoes { csv_separador: sep, csv_decimal: decimal, ..Opcoes::default() };
        let caminho = exportar(&pasta, "o.csv", &opcoes);
        let mut t = Tabela::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
        assert_eq!(t.formato().separador, sep, "separador {sep:?}");
        assert_eq!(t.formato().decimal, decimal, "decimal {decimal:?} com separador {sep:?}");
        let mut esperado = esperado_com_virgula();
        if decimal == '.' {
            for l in &mut esperado {
                l[2] = l[2].replace(',', ".");
            }
        }
        assert_eq!(tudo(&mut t), esperado, "separador {sep:?}, decimal {decimal:?}");
    }
}

#[test]
fn sem_cabecalho_do_exportar() {
    let pasta = Pasta::nova("sem_cab");
    let caminho = exportar(&pasta, "s.csv", &Opcoes { cabecalho: false, ..Opcoes::default() });
    let mut t = Tabela::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
    assert!(!t.formato().cabecalho, "o primeiro registro é dado (1 e um preço numérico)");
    assert_eq!(t.linhas(), 5);
    assert_eq!(tudo(&mut t), esperado_com_virgula());
    assert_eq!(t.nomes(), ["coluna_1", "coluna_2", "coluna_3", "coluna_4"]);
}

#[test]
fn salvar_sem_mudar_nada_nao_regrava() {
    let pasta = Pasta::nova("igual");
    let caminho = exportar(&pasta, "a.csv", &Opcoes::default());
    let original = fs::read(&caminho).unwrap();
    let mut t = Tabela::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
    t.salvar(&mut sem_progresso).unwrap();
    assert_eq!(fs::read(&caminho).unwrap(), original);
}

/// Cada linha regravada pelo `tabular` sai com os mesmos bytes que o Exportar gravou: a mesma regra de aspas,
/// o mesmo fim de linha, o mesmo BOM.
#[test]
fn a_linha_regravada_sai_como_o_exportar_grava() {
    let pasta = Pasta::nova("paridade");
    for opcoes in [
        Opcoes::default(),
        Opcoes { csv_separador: ',', csv_decimal: '.', ..Opcoes::default() },
        Opcoes { csv_separador: '\t', ..Opcoes::default() },
        Opcoes { csv_proteger_formulas: false, ..Opcoes::default() },
    ] {
        let caminho = exportar(&pasta, "p.csv", &opcoes);
        let original = fs::read(&caminho).unwrap();
        let mut t = Tabela::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
        let n = t.linhas();
        // muda uma célula de cada linha e salva; depois volta e salva de novo (a segunda vez regrava as linhas
        // com o texto original, que agora difere do que está no arquivo)
        let guardado: Vec<String> = (0..n).map(|r| t.celula(r, 1).unwrap()).collect();
        for r in 0..n {
            t.editar_celula(r, 1, "temporário").unwrap();
        }
        t.salvar(&mut sem_progresso).unwrap();
        for (r, g) in guardado.iter().enumerate() {
            t.editar_celula(r as u64, 1, g).unwrap();
        }
        let resumo = t.salvar(&mut sem_progresso).unwrap();
        assert_eq!(resumo.linhas_regravadas, n);
        assert_eq!(fs::read(&caminho).unwrap(), original, "opções {opcoes:?}");
    }
}

#[test]
fn regravar_so_o_cabecalho_ou_so_uma_linha_deixa_o_resto_igual() {
    let pasta = Pasta::nova("resto");
    let caminho = exportar(&pasta, "r.csv", &Opcoes::default());
    let original = fs::read(&caminho).unwrap();
    let mut t = Tabela::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
    t.renomear_coluna(1, "produto").unwrap();
    t.salvar(&mut sem_progresso).unwrap();
    let novo = fs::read(&caminho).unwrap();
    let texto_original = String::from_utf8(original.clone()).unwrap();
    assert_eq!(String::from_utf8(novo).unwrap(), texto_original.replacen(";nome;", ";produto;", 1));
    // uma célula da última linha
    let mut t = Tabela::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
    t.editar_celula(4, 3, "outro fim").unwrap();
    t.salvar(&mut sem_progresso).unwrap();
    let novo = String::from_utf8(fs::read(&caminho).unwrap()).unwrap();
    assert!(novo.ends_with("5;004718;7;outro fim\r\n"), "{novo:?}");
    let corte = novo.len() - "5;004718;7;outro fim\r\n".len();
    assert!(novo[..corte].starts_with("\u{FEFF}id;produto;preco;obs\r\n1;"));
}
