//! Codificação de células (§3.1) e coerção das entradas de gravação (§3.3).

use comum::{Afinidade, Celula, Chave, Entrada, LIMITE_TEXTO, Valor};

use super::*;
use crate::{Coagido, coagir as coagir_entrada};

#[test]
fn codificar_celulas() {
    // o `Celula::de_valor` do crate comum, com o limite da grade
    let c = |v: Valor| Celula::de_valor(&v, Some(LIMITE_TEXTO));
    assert_eq!(c(Valor::Nulo), Celula::Nulo);
    assert_eq!(c(Valor::Inteiro((1 << 53) - 1)), ci((1 << 53) - 1));
    assert_eq!(c(Valor::Inteiro(1 << 53)), ci(1 << 53)); // inteiro grande continua exato (sem o {"$int"})
    assert_eq!(c(Valor::Real(1.5)), Celula::Real(1.5));
    assert_eq!(c(Valor::Real(f64::INFINITY)), Celula::Real(f64::INFINITY));
    assert_eq!(c(Valor::Texto("a".repeat(300))), ct(&"a".repeat(300)));
    assert_eq!(c(Valor::Texto("a".repeat(301))), Celula::TextoCortado { inicio: "a".repeat(300), tamanho: 301 });
    assert_eq!(Celula::de_valor(&Valor::Texto("a".repeat(301)), None), ct(&"a".repeat(301)));
    assert_eq!(
        Celula::de_valor(&Valor::Texto("abcdef".into()), Some(3)),
        Celula::TextoCortado { inicio: "abc".into(), tamanho: 6 }
    );
    let dados: Vec<u8> = (0..30).collect();
    assert_eq!(c(Valor::Blob(dados.clone())), Celula::Blob { tamanho: 30, inicio: dados[..24].to_vec() });
    assert_eq!(c(Valor::Blob(vec![])), Celula::Blob { tamanho: 0, inicio: vec![] });
}

fn valor(entrada: Entrada, afinidade: Afinidade) -> Valor {
    match coagir_entrada(&entrada, afinidade).unwrap() {
        Coagido::Valor(v) => v,
        outro => panic!("{outro:?}"),
    }
}

#[test]
fn coagir() {
    use Afinidade::*;
    let i = Valor::Inteiro;
    let r = Valor::Real;
    let casos: Vec<(Entrada, Afinidade, Valor)> = vec![
        (Entrada::Nulo, Integer, Valor::Nulo),
        (tx("12"), Integer, i(12)),
        (tx(" -12 "), Integer, i(-12)),
        (tx("+7"), Integer, i(7)),
        (tx("1,5"), Integer, r(1.5)),
        (tx("1.5"), Integer, r(1.5)),
        (tx("12abc"), Integer, vt("12abc")),
        (tx(""), Integer, Valor::Nulo),
        (tx("   "), Integer, Valor::Nulo),
        (tx("3"), Real, r(3.0)),
        (tx("3,25"), Real, r(3.25)),
        (tx("-0.5"), Real, r(-0.5)),
        (tx("1e3"), Real, r(1000.0)),
        (tx("abc"), Real, vt("abc")),
        (tx(""), Real, Valor::Nulo),
        (tx("10"), Numeric, i(10)),
        (tx("10,5"), Numeric, r(10.5)),
        (tx("10.5"), Numeric, r(10.5)),
        (tx(" x "), Numeric, vt(" x ")),
        (tx(""), Numeric, Valor::Nulo),
        (tx("  12  "), Text, vt("  12  ")),
        (tx(""), Text, vt("")),
        // coluna sem tipo: vira número só a forma exata dele (zeros à esquerda e casas decimais ficam texto)
        (tx("12"), Blob, i(12)),
        (tx("-3"), Blob, i(-3)),
        (tx("2.5"), Blob, r(2.5)),
        (tx("007"), Blob, vt("007")),
        (tx("1.50"), Blob, vt("1.50")),
        (tx(" 6"), Blob, vt(" 6")),
        (tx("-0"), Blob, vt("-0")),
        (tx("1e3"), Blob, vt("1e3")),
        (tx("nan"), Blob, vt("nan")),
        (tx(""), Blob, vt("")),
        // milhar com ponto e vírgula decimal (pt-BR)
        (tx("1.234,56"), Real, r(1234.56)),
        (tx("-12.345.678,9"), Numeric, r(-12345678.9)),
        (tx("1.234,56"), Integer, r(1234.56)),
        (tx("1.23,4"), Real, vt("1.23,4")),
        (tx("1.234"), Real, r(1.234)),
        (tx("99999999999999999999"), Integer, vt("99999999999999999999")),
        (Entrada::Inteiro(42), Text, i(42)),
        (Entrada::Real(2.5), Integer, r(2.5)),
        (Entrada::Inteiro(1), Integer, i(1)),
        (Entrada::Inteiro(9007199254740993), Integer, i(9007199254740993)),
        (Entrada::TextoLiteral("12".into()), Integer, vt("12")),
        (Entrada::Blob(vec![0, 1]), Blob, Valor::Blob(vec![0, 1])),
        (Entrada::Real(f64::INFINITY), Real, r(f64::INFINITY)),
    ];
    for (entrada, afinidade, esperado) in casos {
        assert_eq!(valor(entrada.clone(), afinidade), esperado, "{entrada:?} {afinidade:?}");
    }
}

#[test]
fn coagir_marcas_e_arquivo() {
    let tmp = Pasta::nova("coagir");
    assert_eq!(coagir_entrada(&Entrada::Padrao, Afinidade::Integer).unwrap(), Coagido::Padrao);
    assert_eq!(coagir_entrada(&Entrada::Manter, Afinidade::Text).unwrap(), Coagido::Manter);
    let arquivo = tmp.join("anexo com espaço.bin");
    std::fs::write(&arquivo, b"\x89PNG conteudo").unwrap();
    assert_eq!(valor(Entrada::Arquivo(arquivo), Afinidade::Blob), Valor::Blob(b"\x89PNG conteudo".to_vec()));
    let e = erro(coagir_entrada(&Entrada::Arquivo(tmp.join("sumiu.bin")), Afinidade::Blob));
    assert_eq!(e.status, 404);
    assert!(e.mensagem.starts_with("Não foi possível ler o arquivo escolhido."), "{}", e.mensagem);
}

#[test]
fn milhar_com_ponto_e_virgula_decimal() {
    let ex = exemplo("milhar");
    ex.banco.atualizar("clientes", &Chave::Rowid(1), &vals(&[("limite", tx("1.234,56"))])).unwrap();
    assert_eq!(
        cru(&ex.caminho, "SELECT typeof(limite), limite FROM clientes WHERE id = 1", &[]),
        vec![vec![vt("real"), Valor::Real(1234.56)]]
    );
}
