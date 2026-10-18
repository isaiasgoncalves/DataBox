//! A gravação de um registro: aspas, separador e codificação do [`Formato`].

use super::formato::{EstiloAspas, Formato};
use crate::numero::{TipoDetectado, classificar};

/// O campo precisa de aspas, qualquer que seja o estilo: tem o separador, aspas ou quebra de linha.
fn tem_especial(campo: &str, sep: u8) -> bool {
    campo.bytes().any(|b| b == sep || b == b'"' || b == b'\n' || b == b'\r')
}

fn e_numero(campo: &str, decimal: char) -> bool {
    matches!(classificar(campo, decimal), TipoDetectado::Inteiro | TipoDetectado::Decimal) && campo.trim() == campo
}

/// O campo leva aspas neste formato? `unico` é o registro de um campo só: se for vazio, sai `""` (uma linha em
/// branco seria outra coisa para quem lê), como o Exportar.
pub(crate) fn leva_aspas(campo: &str, formato: &Formato, unico: bool) -> bool {
    if unico && campo.is_empty() {
        return true;
    }
    if tem_especial(campo, formato.sep()) {
        return true;
    }
    match formato.aspas {
        EstiloAspas::Minimas => false,
        EstiloAspas::Todas => true,
        EstiloAspas::NaoNumericos => !e_numero(campo, formato.decimal),
    }
}

/// Acrescenta os campos de um registro, já codificados, sem a quebra de linha. `Err` traz o primeiro caractere
/// que a codificação não tem.
pub fn escrever_registro<S: AsRef<str>>(saida: &mut Vec<u8>, campos: &[S], formato: &Formato) -> Result<(), char> {
    let sep = formato.sep();
    let unico = campos.len() == 1;
    for (i, campo) in campos.iter().enumerate() {
        if i > 0 {
            saida.push(sep);
        }
        let campo = campo.as_ref();
        if leva_aspas(campo, formato, unico) {
            saida.push(b'"');
            for (k, parte) in campo.split('"').enumerate() {
                if k > 0 {
                    saida.extend_from_slice(b"\"\"");
                }
                formato.codificacao.anexar(saida, parte)?;
            }
            saida.push(b'"');
        } else {
            formato.codificacao.anexar(saida, campo)?;
        }
    }
    Ok(())
}

/// O registro, com a quebra de linha do formato, como bytes.
pub fn registro_em_bytes<S: AsRef<str>>(campos: &[S], formato: &Formato) -> Result<Vec<u8>, char> {
    let mut saida = Vec::new();
    escrever_registro(&mut saida, campos, formato)?;
    saida.extend_from_slice(formato.fim_de_linha.bytes());
    Ok(saida)
}

#[cfg(test)]
mod testes {
    use super::super::codificacao::Codificacao;
    use super::super::formato::FimDeLinha;
    use super::*;

    fn formato(aspas: EstiloAspas) -> Formato {
        Formato { aspas, ..Formato::padrao() }
    }

    fn texto(campos: &[&str], f: &Formato) -> String {
        String::from_utf8(registro_em_bytes(campos, f).unwrap()).unwrap()
    }

    #[test]
    fn aspas_minimas_como_o_exportar() {
        let f = formato(EstiloAspas::Minimas);
        assert_eq!(texto(&["a", "b c", ""], &f), "a;b c;\r\n");
        assert_eq!(texto(&["a;b", "c"], &f), "\"a;b\";c\r\n");
        assert_eq!(texto(&["ele disse \"oi\"", "x"], &f), "\"ele disse \"\"oi\"\"\";x\r\n");
        assert_eq!(texto(&["l1\nl2", "x"], &f), "\"l1\nl2\";x\r\n");
        assert_eq!(texto(&["l1\rl2"], &f), "\"l1\rl2\"\r\n");
        assert_eq!(texto(&["a,b", "1,5"], &f), "a,b;1,5\r\n", "a vírgula só pede aspas no separador `,`");
        assert_eq!(texto(&[""], &f), "\"\"\r\n", "registro de um campo vazio");
        assert_eq!(texto(&["", ""], &f), ";\r\n");
        assert_eq!(texto(&["x"], &f), "x\r\n");
    }

    #[test]
    fn outros_separadores_e_fim_de_linha() {
        let f = Formato { separador: ',', fim_de_linha: FimDeLinha::Lf, ..Formato::padrao() };
        assert_eq!(texto(&["a,b", "1,5", "c;d"], &f), "\"a,b\",\"1,5\",c;d\n");
        let f = Formato { separador: '\t', fim_de_linha: FimDeLinha::Cr, ..Formato::padrao() };
        assert_eq!(texto(&["a\tb", "c"], &f), "\"a\tb\"\tc\r");
    }

    #[test]
    fn todas_e_nao_numericos() {
        let todas = formato(EstiloAspas::Todas);
        assert_eq!(texto(&["a", "1", ""], &todas), "\"a\";\"1\";\"\"\r\n");
        let nn = formato(EstiloAspas::NaoNumericos);
        assert_eq!(texto(&["a", "12", "1,5", "004718", ""], &nn), "\"a\";12;1,5;\"004718\";\"\"\r\n");
        let nn_ponto = Formato { decimal: '.', separador: ',', ..formato(EstiloAspas::NaoNumericos) };
        assert_eq!(texto(&["a", "1.5", "x y"], &nn_ponto), "\"a\",1.5,\"x y\"\r\n");
    }

    #[test]
    fn codificacao_na_gravacao() {
        let f = Formato { codificacao: Codificacao::Windows1252, ..Formato::padrao() };
        let b = registro_em_bytes(&["ação", "\"é\"", "€"], &f).unwrap();
        assert_eq!(b.as_slice(), b"a\xE7\xE3o;\"\"\"\xE9\"\"\";\x80\r\n".as_slice());
        let utf8 = Formato { codificacao: Codificacao::Utf8, ..Formato::padrao() };
        assert_eq!(registro_em_bytes(&["ação"], &utf8).unwrap(), "ação\r\n".as_bytes());
        assert_eq!(registro_em_bytes(&["a", "日"], &f), Err('日'));
    }
}
