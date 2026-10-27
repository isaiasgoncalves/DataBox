//! Os testes do `t!`, do `tn!`, do idioma atual e dos catálogos embutidos. Cada um roda com [`com_idioma`]: o idioma
//! global é do processo, e o `cargo test` roda os testes em paralelo.

use super::*;

#[test]
fn todos_os_catalogos_embutidos_abrem() {
    for idioma in IDIOMAS {
        if let Some(texto) = idioma.catalogo {
            let cat = po::ler(texto).unwrap_or_else(|e| panic!("{}.po: {e}", idioma.codigo));
            assert_eq!(cat.idioma.as_deref(), Some(idioma.codigo), "o Language do {}.po", idioma.codigo);
            assert!(!cat.traducoes.is_empty());
        }
    }
    // o português não tem catálogo (a chave é o texto)
    assert!(PORTUGUES.catalogo.is_none());
}

#[test]
fn registro_sem_codigos_repetidos_e_com_o_portugues_primeiro() {
    assert_eq!(IDIOMAS[0], PORTUGUES, "o índice 0 é o padrão antes de qualquer definir");
    for (i, a) in IDIOMAS.iter().enumerate() {
        assert!(IDIOMAS[i + 1..].iter().all(|b| !b.codigo.eq_ignore_ascii_case(a.codigo)), "{}", a.codigo);
        assert_ne!(a.milhar, a.decimal, "{}", a.codigo);
    }
}

#[test]
fn t_sem_e_com_argumentos() {
    com_idioma(PORTUGUES, || {
        assert_eq!(t!("Abrir pasta"), "Abrir pasta");
        assert!(matches!(t!("Abrir pasta"), Cow::Borrowed(_)), "sem argumentos não aloca");
        let tabela = String::from("vendas");
        assert_eq!(
            t!("Gravado em {tabela} (linha {linha})", tabela = tabela, linha = 3),
            "Gravado em vendas (linha 3)"
        );
        // a vírgula do fim e argumentos que são expressões
        assert_eq!(t!("{a}+{b}", a = 1 + 1, b = tabela.len(),), "2+6");
    });
    com_idioma(INGLES, || {
        assert_eq!(t!("Abrir pasta"), "Open folder");
        let tabela = "vendas";
        assert_eq!(t!("Gravado em {tabela} (linha {linha})", tabela = tabela, linha = 3), "Saved to vendas (row 3)");
    });
}

#[test]
fn chave_ausente_volta_o_portugues() {
    com_idioma(INGLES, || {
        assert_eq!(t!("Um texto que o en.po não tem"), "Um texto que o en.po não tem");
        assert_eq!(t!("Ainda sem {coisa}", coisa = "tradução"), "Ainda sem tradução");
        assert_eq!(tn!(2, "{n} coisa nova", "{n} coisas novas"), "2 coisas novas");
        assert_eq!(traduzir("Nada"), "Nada");
    });
}

#[test]
fn plural_do_portugues() {
    com_idioma(PORTUGUES, || {
        assert_eq!(tn!(0, "{n} linha", "{n} linhas"), "0 linhas");
        assert_eq!(tn!(1, "{n} linha", "{n} linhas"), "1 linha");
        assert_eq!(tn!(2, "{n} linha", "{n} linhas"), "2 linhas");
        assert_eq!(tn!(1_234_567u64, "{n} linha", "{n} linhas"), "1.234.567 linhas");
        // negativos contam pelo absoluto; a contagem pode vir por referência
        assert_eq!(tn!(-1i64, "{n} linha", "{n} linhas"), "1 linha");
        let n = 5usize;
        assert_eq!(tn!(&n, "{n} linha", "{n} linhas"), "5 linhas");
        // o singular sem {n}, e outros argumentos
        assert_eq!(tn!(1, "Uma linha em {t}", "{n} linhas em {t}", t = "vendas"), "Uma linha em vendas");
        assert_eq!(tn!(3, "Uma linha em {t}", "{n} linhas em {t}", t = "vendas"), "3 linhas em vendas");
        assert!(matches!(tn!(1, "Uma", "{n}"), Cow::Borrowed("Uma")), "sem buraco não aloca");
    });
}

#[test]
fn plural_do_ingles() {
    com_idioma(INGLES, || {
        assert_eq!(tn!(0, "{n} linha", "{n} linhas"), "0 rows");
        assert_eq!(tn!(1, "{n} linha", "{n} linhas"), "1 row");
        assert_eq!(tn!(1234, "{n} linha", "{n} linhas"), "1,234 rows");
        assert_eq!(tn!(1, "Uma linha em {t}", "{n} linhas em {t}", t = "sales"), "One row in sales");
        assert_eq!(tn!(7, "Uma linha em {t}", "{n} linhas em {t}", t = "sales"), "7 rows in sales");
        assert_eq!(escolher_plural(2, "linha", "linhas"), "rows");
        assert_eq!(escolher_plural(1, "tabela", "tabelas"), "tabela", "sem a entrada, o português");
    });
}

#[test]
fn t_estatico_e_traduzir() {
    const SALVAR: &str = t_estatico!("Salvar");
    static TABELA: [&str; 2] = [t_estatico!("Salvar"), t_estatico!("Abrir pasta")];
    com_idioma(PORTUGUES, || assert_eq!(traduzir(SALVAR), "Salvar"));
    com_idioma(INGLES, || {
        assert_eq!(traduzir(SALVAR), "Save");
        assert_eq!(TABELA.map(traduzir), ["Save", "Open folder"]);
    });
}

#[test]
fn contagem_com_o_milhar_do_idioma() {
    com_idioma(PORTUGUES, || {
        assert_eq!(
            [0, 7, 999, 1000, 65_536, u64::MAX].map(contagem),
            ["0", "7", "999", "1.000", "65.536", "18.446.744.073.709.551.615"]
        );
    });
    com_idioma(INGLES, || assert_eq!(contagem(1_000_000), "1,000,000"));
}

#[test]
fn isolar_separa_a_thread_e_volta_no_fim() {
    let global = atual();
    com_idioma(INGLES, || {
        assert_eq!(atual(), INGLES);
        // o definir de dentro só mexe nesta thread
        definir(PORTUGUES);
        assert_eq!(atual(), PORTUGUES);
        // isolar dentro de isolar começa do que vale
        isolar(|| {
            assert_eq!(atual(), PORTUGUES);
            definir(INGLES);
        });
        assert_eq!(atual(), PORTUGUES, "o de dentro voltou");
    });
    assert_eq!(atual(), global);
    // mesmo com pânico
    let r = std::panic::catch_unwind(|| com_idioma(INGLES, || panic!("de propósito")));
    assert!(r.is_err());
    assert_eq!(atual(), global);
}

#[test]
fn escolha_de_e_para_texto() {
    assert_eq!(Escolha::de_texto("system"), Some(Escolha::Sistema));
    assert_eq!(Escolha::de_texto("pt-BR"), Some(Escolha::Idioma(PORTUGUES)));
    assert_eq!(Escolha::de_texto("pt_br"), Some(Escolha::Idioma(PORTUGUES)));
    assert_eq!(Escolha::de_texto("en"), Some(Escolha::Idioma(INGLES)));
    for estranho in ["", "fr", "System", "português", "en-US"] {
        assert_eq!(Escolha::de_texto(estranho), None, "{estranho}");
    }
    for e in Escolha::todas() {
        assert_eq!(Escolha::de_texto(e.como_str()), Some(e));
    }
    assert_eq!(Escolha::todas().len(), IDIOMAS.len() + 1);
    assert_eq!(Escolha::default(), Escolha::Sistema);
    assert_eq!(Escolha::Idioma(INGLES).resolver(), INGLES);
    assert!(IDIOMAS.contains(&Escolha::Sistema.resolver()));
}

#[test]
fn formatos_dos_idiomas() {
    assert_eq!((PORTUGUES.milhar(), PORTUGUES.decimal(), PORTUGUES.padrao_data()), ('.', ',', "%d/%m/%Y"));
    assert_eq!((INGLES.milhar(), INGLES.decimal(), INGLES.padrao_data()), (',', '.', "%Y-%m-%d"));
    assert_eq!(INGLES.to_string(), "English");
    assert_eq!(PORTUGUES.nome_curto(), "Português");
    assert_eq!(Idioma::de_codigo(" EN "), Some(INGLES));
}

#[test]
fn usa_acha_o_nome_entre_chaves() {
    assert!(__usa("Gravado em {tabela}", "tabela"));
    assert!(__usa("{n:>3} linhas", "n"));
    assert!(!__usa("Gravado em {tabelas}", "tabela"));
    assert!(!__usa("Gravado em tabela", "tabela"));
    assert!(!__usa("{tab", "tab"));
    assert!(!__usa("", "n"));
}
