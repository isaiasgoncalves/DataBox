//! A troca dos `{nome}` de um texto pelos argumentos do `t!`/`tn!`.

use std::fmt::{self, Write};

/// Um argumento nomeado: o nome do `{nome}` e o valor.
pub type Argumento<'a> = (&'a str, &'a dyn fmt::Display);

/// Troca cada `{nome}` de `modelo` pelo argumento de mesmo nome. `{{` e `}}` viram `{` e `}` (como no `format!`).
/// Um `{nome}` sem argumento fica como está (o texto aparece, com o buraco à vista, em vez de sumir); o que vem
/// depois de `:` dentro das chaves (`{n:>3}`) é ignorado: o valor sai com o `Display` dele.
pub fn preencher(modelo: &str, args: &[Argumento<'_>]) -> String {
    let mut saida = String::with_capacity(modelo.len() + 16);
    let mut resto = modelo;
    while let Some(i) = resto.find(['{', '}']) {
        saida.push_str(&resto[..i]);
        let depois = &resto[i + 1..];
        if resto[i..].starts_with("{{") {
            saida.push('{');
            resto = &resto[i + 2..];
            continue;
        }
        if resto[i..].starts_with("}}") {
            saida.push('}');
            resto = &resto[i + 2..];
            continue;
        }
        if resto[i..].starts_with('}') {
            saida.push('}');
            resto = depois;
            continue;
        }
        let Some(fim) = depois.find('}') else {
            saida.push_str(&resto[i..]);
            return saida;
        };
        let dentro = &depois[..fim];
        let nome = dentro.split(':').next().unwrap_or(dentro).trim();
        match args.iter().find(|(n, _)| *n == nome) {
            Some((_, valor)) => {
                let _ = write!(saida, "{valor}");
            }
            None => {
                saida.push('{');
                saida.push_str(dentro);
                saida.push('}');
            }
        }
        resto = &depois[fim + 1..];
    }
    saida.push_str(resto);
    saida
}

/// Os nomes dos `{nome}` de um texto, na ordem (sem `{{`/`}}`): o extrator compara os da chave com os da tradução.
pub fn nomes(modelo: &str) -> Vec<&str> {
    let mut saida = Vec::new();
    let mut resto = modelo;
    while let Some(i) = resto.find('{') {
        if resto[i..].starts_with("{{") {
            resto = &resto[i + 2..];
            continue;
        }
        let depois = &resto[i + 1..];
        let Some(fim) = depois.find('}') else { break };
        let dentro = &depois[..fim];
        saida.push(dentro.split(':').next().unwrap_or(dentro).trim());
        resto = &depois[fim + 1..];
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn preenche_os_nomes() {
        let (t, l) = ("vendas", 12);
        let args: [Argumento; 2] = [("tabela", &t), ("linha", &l)];
        assert_eq!(preencher("Gravado em {tabela} (linha {linha})", &args), "Gravado em vendas (linha 12)");
        assert_eq!(preencher("{linha}{linha}", &args), "1212");
        assert_eq!(preencher("sem nada", &args), "sem nada");
        assert_eq!(preencher("", &args), "");
    }

    #[test]
    fn chaves_duplas_buracos_e_formato() {
        let n = 3;
        let args: [Argumento; 1] = [("n", &n)];
        assert_eq!(preencher("{{n}} = {n}", &args), "{n} = 3");
        assert_eq!(preencher("a }} b", &args), "a } b");
        assert_eq!(preencher("{faltou} {n}", &args), "{faltou} 3");
        assert_eq!(preencher("{n:>5}", &args), "3");
        assert_eq!(preencher("aberto {n", &args), "aberto {n");
        assert_eq!(preencher("solta } aqui", &args), "solta } aqui");
    }

    #[test]
    fn lista_os_nomes() {
        assert_eq!(nomes("{a} e {b:>2} e {{c}} e {a}"), ["a", "b", "a"]);
        assert!(nomes("nada {").is_empty());
    }
}
