//! A cola de colunas: o cabeçalho do texto colado, a cola por posição com as quantidades diferentes, o
//! "atualizar uma coluna que existe" e os nomes das colunas novas.

use crate::numero::{TipoDetectado, classificar, tipo_da_coluna};

use super::tsv::{coluna_do_texto, largura};

/// De onde veio o texto colado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrigemDoTexto {
    /// O texto que o próprio DataBox copiou por último: a primeira linha é o cabeçalho, sempre.
    Databox,
    /// Texto de fora (o Excel, um editor): vale a heurística de [`texto_tem_cabecalho`].
    Externo,
}

/// A primeira linha do texto colado é o nome das colunas?
///
/// - Do DataBox: sim (o "Copiar coluna" sempre leva o nome).
/// - De fora: só quando todas as células da primeira linha são texto (ou vazias) e há valores embaixo que são
///   número, data ou booleano em pelo menos uma coluna. Texto sobre texto não conta: sem a certeza, a coluna
///   nasce como `coluna_N`, e o nome sugerido fica a cargo da pessoa (que o edita ao colar). Uma linha só nunca
///   é cabeçalho.
pub fn texto_tem_cabecalho(linhas: &[Vec<String>], origem: OrigemDoTexto, decimal: char) -> bool {
    if origem == OrigemDoTexto::Databox {
        return !linhas.is_empty();
    }
    if linhas.len() < 2 {
        return false;
    }
    let primeira = &linhas[0];
    if primeira.iter().any(|c| !matches!(classificar(c, decimal), TipoDetectado::Texto | TipoDetectado::Vazio)) {
        return false;
    }
    (0..largura(linhas)).any(|j| {
        let nome = primeira.get(j).map_or("", String::as_str);
        classificar(nome, decimal) == TipoDetectado::Texto
            && tipo_da_coluna(linhas[1..].iter().map(|l| l.get(j).map_or("", String::as_str)), decimal).e_tipado()
    })
}

/// Uma coluna do texto colado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColunaColada {
    /// O nome vindo da primeira linha; `None` sem cabeçalho (ou cabeçalho vazio): quem cola dá `coluna_N`.
    pub nome: Option<String>,
    pub valores: Vec<String>,
}

/// As colunas do texto colado, uma por campo da linha mais larga. Com cabeçalho (ver [`texto_tem_cabecalho`]),
/// a primeira linha dá os nomes e fica fora dos valores.
pub fn colunas_coladas(linhas: &[Vec<String>], origem: OrigemDoTexto, decimal: char) -> Vec<ColunaColada> {
    let cabecalho = texto_tem_cabecalho(linhas, origem, decimal);
    (0..largura(linhas))
        .map(|j| {
            let mut valores = coluna_do_texto(linhas, j);
            let nome = if cabecalho {
                let n = valores.remove(0);
                Some(n).filter(|n| !n.trim().is_empty())
            } else {
                None
            };
            ColunaColada { nome, valores }
        })
        .collect()
}

/// Como o número de valores colados se compara ao de linhas do arquivo (o aviso "A coluna colada tem 1.204
/// valores e o arquivo, 1.180 linhas.").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantidades {
    Iguais,
    /// Valores a mais que linhas: `extras` valores não têm linha (o aviso pergunta "Colar 1.180" ou
    /// "Acrescentar 24 linhas").
    SobramValores {
        extras: usize,
    },
    /// Linhas a mais que valores: `vazias` linhas ficam vazias (o aviso só informa).
    FaltamValores {
        vazias: usize,
    },
}

pub fn comparar_quantidades(valores: usize, linhas_do_arquivo: usize) -> Quantidades {
    match valores.cmp(&linhas_do_arquivo) {
        std::cmp::Ordering::Equal => Quantidades::Iguais,
        std::cmp::Ordering::Greater => Quantidades::SobramValores { extras: valores - linhas_do_arquivo },
        std::cmp::Ordering::Less => Quantidades::FaltamValores { vazias: linhas_do_arquivo - valores },
    }
}

/// O que fazer com os valores que não têm linha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuandoSobram {
    /// "Colar 1.180": os valores a mais ficam de fora.
    Cortar,
    /// "Acrescentar 24 linhas": o arquivo ganha as linhas que faltam.
    Acrescentar,
}

/// Os valores colados já na medida das linhas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posicionamento {
    /// Um valor por linha, a linha 1 colada na linha 1 do arquivo; com `Acrescentar`, também um por linha nova.
    pub valores: Vec<String>,
    /// Linhas a acrescentar ao arquivo (só com `Acrescentar`, e só se sobraram valores).
    pub linhas_a_acrescentar: usize,
    /// Linhas que ficaram vazias porque faltaram valores.
    pub linhas_vazias: usize,
    /// Valores que ficaram de fora (só com `Cortar`).
    pub descartados: usize,
}

/// A cola por posição: o valor `i` vai para a linha `i`.
pub fn posicionar(valores: &[String], linhas_do_arquivo: usize, sobram: QuandoSobram) -> Posicionamento {
    match comparar_quantidades(valores.len(), linhas_do_arquivo) {
        Quantidades::Iguais => {
            Posicionamento { valores: valores.to_vec(), linhas_a_acrescentar: 0, linhas_vazias: 0, descartados: 0 }
        }
        Quantidades::FaltamValores { vazias } => {
            let mut v = valores.to_vec();
            v.resize(linhas_do_arquivo, String::new());
            Posicionamento { valores: v, linhas_a_acrescentar: 0, linhas_vazias: vazias, descartados: 0 }
        }
        Quantidades::SobramValores { extras } => match sobram {
            QuandoSobram::Cortar => Posicionamento {
                valores: valores[..linhas_do_arquivo].to_vec(),
                linhas_a_acrescentar: 0,
                linhas_vazias: 0,
                descartados: extras,
            },
            QuandoSobram::Acrescentar => Posicionamento {
                valores: valores.to_vec(),
                linhas_a_acrescentar: extras,
                linhas_vazias: 0,
                descartados: 0,
            },
        },
    }
}

/// Como atualizar uma coluna que já existe com os valores trazidos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoAtualizacao {
    /// "substituir": onde há valor trazido, ele vale.
    Substituir,
    /// "só as células vazias": só onde a célula está vazia (ou só com espaços).
    SoCelulasVazias,
}

/// O resultado de atualizar uma coluna.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Atualizacao {
    pub valores: Vec<String>,
    /// As linhas (de 0) cuja célula mudou: o que a prévia tinge.
    pub alteradas: Vec<usize>,
}

/// Atualiza a coluna `atual` com os `trazidos` (uma entrada por linha; `None` é a linha sem par, que nunca
/// apaga o que já está). Linhas além do fim de `atual` ou de `trazidos` ficam como estão.
pub fn atualizar_coluna(atual: &[String], trazidos: &[Option<String>], modo: ModoAtualizacao) -> Atualizacao {
    let mut valores = atual.to_vec();
    let mut alteradas = Vec::new();
    for (i, (velho, novo)) in atual.iter().zip(trazidos).enumerate() {
        let Some(novo) = novo else {
            continue;
        };
        let vale = match modo {
            ModoAtualizacao::Substituir => true,
            ModoAtualizacao::SoCelulasVazias => velho.trim().is_empty(),
        };
        if vale && novo != velho {
            valores[i] = novo.clone();
            alteradas.push(i);
        }
    }
    Atualizacao { valores, alteradas }
}

fn existe<S: AsRef<str>>(nome: &str, existentes: &[S]) -> bool {
    let n = nome.trim().to_lowercase();
    existentes.iter().any(|e| e.as_ref().trim().to_lowercase() == n)
}

/// O nome `coluna_N` de uma coluna sem nome, a partir de `posicao` (a posição de 1 da coluna nova): o primeiro
/// que ainda não existe.
pub fn nome_generico<S: AsRef<str>>(posicao: usize, existentes: &[S]) -> String {
    let mut n = posicao.max(1);
    loop {
        let nome = format!("coluna_{n}");
        if !existe(&nome, existentes) {
            return nome;
        }
        n += 1;
    }
}

/// O nome com que uma coluna da origem entra no destino: o dela; se já existe (sem diferenciar maiúsculas),
/// `preco (catalogo)`, com o `rotulo` da origem; e se ainda colidir, `preco (catalogo) 2`, `3`…
pub fn nome_da_coluna_trazida<S: AsRef<str>>(nome: &str, rotulo: &str, existentes: &[S]) -> String {
    if !existe(nome, existentes) {
        return nome.to_string();
    }
    let base = format!("{} ({rotulo})", nome.trim());
    if !existe(&base, existentes) {
        return base;
    }
    (2..).map(|n| format!("{base} {n}")).find(|c| !existe(c, existentes)).unwrap_or(base)
}

/// O rótulo de uma origem para os nomes das colunas: o nome do arquivo sem a extensão e sem o ano no fim
/// (`catalogo_2025.csv` → `catalogo`).
pub fn rotulo_da_origem(nome_do_arquivo: &str) -> String {
    let sem_extensao = match nome_do_arquivo.rsplit_once('.') {
        Some((resto, ext)) if !resto.is_empty() && ext.len() <= 5 && ext.bytes().all(|b| b.is_ascii_alphanumeric()) => {
            resto
        }
        _ => nome_do_arquivo,
    };
    let bytes = sem_extensao.as_bytes();
    if bytes.len() > 5 {
        let (resto, ano) = bytes.split_at(bytes.len() - 4);
        if ano.iter().all(u8::is_ascii_digit) && matches!(resto.last(), Some(b'_' | b'-' | b' ')) {
            let limpo = sem_extensao[..resto.len() - 1].trim_end();
            if !limpo.is_empty() {
                return limpo.to_string();
            }
        }
    }
    sem_extensao.to_string()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn t(linhas: &[&[&str]]) -> Vec<Vec<String>> {
        linhas.iter().map(|l| l.iter().map(|c| c.to_string()).collect()).collect()
    }

    fn v(textos: &[&str]) -> Vec<String> {
        textos.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn databox_sempre_tem_cabecalho() {
        assert!(texto_tem_cabecalho(&t(&[&["regiao"], &["Sul"], &["Norte"]]), OrigemDoTexto::Databox, ','));
        assert!(texto_tem_cabecalho(&t(&[&["1"], &["2"]]), OrigemDoTexto::Databox, ','));
        assert!(!texto_tem_cabecalho(&[], OrigemDoTexto::Databox, ','));
    }

    #[test]
    fn externo_texto_sobre_numeros_ou_datas_e_cabecalho() {
        assert!(texto_tem_cabecalho(&t(&[&["valor"], &["12,90"], &["178,00"]]), OrigemDoTexto::Externo, ','));
        assert!(texto_tem_cabecalho(&t(&[&["data"], &["2025-01-02"], &["2025-01-03"]]), OrigemDoTexto::Externo, ','));
        assert!(texto_tem_cabecalho(&t(&[&["ok"], &["true"], &["false"]]), OrigemDoTexto::Externo, ','));
    }

    #[test]
    fn externo_sem_certeza_nao_vira_cabecalho() {
        assert!(
            !texto_tem_cabecalho(&t(&[&["Sul"], &["Norte"], &["Sul"]]), OrigemDoTexto::Externo, ','),
            "texto sobre texto"
        );
        assert!(
            !texto_tem_cabecalho(&t(&[&["12,90"], &["178,00"]]), OrigemDoTexto::Externo, ','),
            "número sobre número"
        );
        assert!(!texto_tem_cabecalho(&t(&[&["valor"]]), OrigemDoTexto::Externo, ','), "uma linha só");
        assert!(!texto_tem_cabecalho(&[], OrigemDoTexto::Externo, ','));
        assert!(
            !texto_tem_cabecalho(&t(&[&["valor"], &["12"], &["abc"]]), OrigemDoTexto::Externo, ','),
            "valores misturados"
        );
        assert!(
            !texto_tem_cabecalho(&t(&[&["10", "x"], &["1", "2"]]), OrigemDoTexto::Externo, ','),
            "um número na primeira linha"
        );
    }

    #[test]
    fn externo_celula_vazia_na_primeira_linha_nao_impede() {
        assert!(texto_tem_cabecalho(&t(&[&["", "x"], &["1", "2"]]), OrigemDoTexto::Externo, ','));
    }

    #[test]
    fn externo_com_varias_colunas_decide_pela_tabela() {
        let tabela = t(&[&["produto", "preco"], &["Caneta", "4,50"], &["Lápis", "1,20"]]);
        assert!(texto_tem_cabecalho(&tabela, OrigemDoTexto::Externo, ','));
        let c = colunas_coladas(&tabela, OrigemDoTexto::Externo, ',');
        assert_eq!(c.len(), 2);
        assert_eq!(c[0], ColunaColada { nome: Some("produto".into()), valores: v(&["Caneta", "Lápis"]) });
        assert_eq!(c[1], ColunaColada { nome: Some("preco".into()), valores: v(&["4,50", "1,20"]) });
        // primeira linha com um número: nenhuma coluna tem nome
        let sem = t(&[&["10", "preco"], &["1", "2"]]);
        let c = colunas_coladas(&sem, OrigemDoTexto::Externo, ',');
        assert_eq!(c[0].nome, None);
        assert_eq!(c[0].valores, ["10", "1"]);
    }

    #[test]
    fn colunas_coladas_do_databox_e_linhas_irregulares() {
        let c = colunas_coladas(&t(&[&["regiao"], &["Sul"], &[""], &["Norte"]]), OrigemDoTexto::Databox, ',');
        assert_eq!(c, vec![ColunaColada { nome: Some("regiao".into()), valores: v(&["Sul", "", "Norte"]) }]);
        let c = colunas_coladas(&t(&[&["a", "b"], &["1"], &["2", "3"]]), OrigemDoTexto::Databox, ',');
        assert_eq!(c[1].valores, ["", "3"], "linha curta: vazio");
        let c = colunas_coladas(&t(&[&["", "b"], &["1", "2"]]), OrigemDoTexto::Databox, ',');
        assert_eq!(c[0].nome, None, "cabeçalho vazio");
        assert!(colunas_coladas(&[], OrigemDoTexto::Externo, ',').is_empty());
    }

    #[test]
    fn quantidades() {
        assert_eq!(comparar_quantidades(1180, 1180), Quantidades::Iguais);
        assert_eq!(comparar_quantidades(1204, 1180), Quantidades::SobramValores { extras: 24 });
        assert_eq!(comparar_quantidades(1156, 1180), Quantidades::FaltamValores { vazias: 24 });
        assert_eq!(comparar_quantidades(0, 0), Quantidades::Iguais);
    }

    #[test]
    fn posicionar_com_a_mesma_quantidade() {
        let p = posicionar(&v(&["a", "b", "c"]), 3, QuandoSobram::Cortar);
        assert_eq!(
            p,
            Posicionamento { valores: v(&["a", "b", "c"]), linhas_a_acrescentar: 0, linhas_vazias: 0, descartados: 0 }
        );
    }

    #[test]
    fn posicionar_com_valores_a_mais() {
        let valores = v(&["a", "b", "c", "d"]);
        let corta = posicionar(&valores, 2, QuandoSobram::Cortar);
        assert_eq!(corta.valores, ["a", "b"]);
        assert_eq!((corta.descartados, corta.linhas_a_acrescentar, corta.linhas_vazias), (2, 0, 0));
        let acrescenta = posicionar(&valores, 2, QuandoSobram::Acrescentar);
        assert_eq!(acrescenta.valores, ["a", "b", "c", "d"]);
        assert_eq!((acrescenta.descartados, acrescenta.linhas_a_acrescentar, acrescenta.linhas_vazias), (0, 2, 0));
    }

    #[test]
    fn posicionar_com_valores_a_menos_deixa_vazias_nas_duas_politicas() {
        for sobram in [QuandoSobram::Cortar, QuandoSobram::Acrescentar] {
            let p = posicionar(&v(&["a"]), 3, sobram);
            assert_eq!(p.valores, ["a", "", ""]);
            assert_eq!((p.linhas_vazias, p.linhas_a_acrescentar, p.descartados), (2, 0, 0));
        }
        let vazio = posicionar(&[], 2, QuandoSobram::Cortar);
        assert_eq!(vazio.valores, ["", ""]);
    }

    #[test]
    fn atualizar_substituindo() {
        let atual = v(&["Sul", "", "Norte", "Sul"]);
        let trazidos = vec![Some("Sudeste".to_string()), Some("Centro".to_string()), None, Some("Sul".to_string())];
        let a = atualizar_coluna(&atual, &trazidos, ModoAtualizacao::Substituir);
        assert_eq!(a.valores, ["Sudeste", "Centro", "Norte", "Sul"], "sem par não apaga");
        assert_eq!(a.alteradas, vec![0, 1], "o que já era igual não conta");
    }

    #[test]
    fn atualizar_so_as_vazias() {
        let atual = v(&["Sul", "", "  ", "Norte"]);
        let trazidos = vec![Some("X".to_string()), Some("Y".to_string()), Some("Z".to_string()), None];
        let a = atualizar_coluna(&atual, &trazidos, ModoAtualizacao::SoCelulasVazias);
        assert_eq!(a.valores, ["Sul", "Y", "Z", "Norte"]);
        assert_eq!(a.alteradas, vec![1, 2]);
    }

    #[test]
    fn atualizar_com_tamanhos_diferentes() {
        let a = atualizar_coluna(&v(&["a", "b", "c"]), &[Some("X".to_string())], ModoAtualizacao::Substituir);
        assert_eq!(a.valores, ["X", "b", "c"]);
        let a =
            atualizar_coluna(&v(&["a"]), &[Some("X".to_string()), Some("Y".to_string())], ModoAtualizacao::Substituir);
        assert_eq!(a.valores, ["X"]);
    }

    #[test]
    fn nomes_genericos() {
        assert_eq!(nome_generico(4, &v(&["a", "b", "c"])), "coluna_4");
        assert_eq!(nome_generico(4, &v(&["coluna_4", "COLUNA_5"])), "coluna_6");
        assert_eq!(nome_generico(0, &Vec::<String>::new()), "coluna_1");
    }

    #[test]
    fn nome_da_coluna_trazida_evita_colisao() {
        let existentes = v(&["codigo", "Preco"]);
        assert_eq!(nome_da_coluna_trazida("categoria", "catalogo", &existentes), "categoria");
        assert_eq!(nome_da_coluna_trazida("preco", "catalogo", &existentes), "preco (catalogo)");
        assert_eq!(nome_da_coluna_trazida("codigo ", "catalogo", &existentes), "codigo (catalogo)");
        let mais = v(&["preco", "preco (catalogo)"]);
        assert_eq!(nome_da_coluna_trazida("preco", "catalogo", &mais), "preco (catalogo) 2");
        let ainda = v(&["preco", "preco (catalogo)", "preco (catalogo) 2"]);
        assert_eq!(nome_da_coluna_trazida("PRECO", "catalogo", &ainda), "PRECO (catalogo) 3");
    }

    #[test]
    fn rotulo_do_arquivo_de_origem() {
        assert_eq!(rotulo_da_origem("catalogo_2025.csv"), "catalogo");
        assert_eq!(rotulo_da_origem("vendas-2025.csv"), "vendas");
        assert_eq!(rotulo_da_origem("estoque 2026.xlsx"), "estoque");
        assert_eq!(rotulo_da_origem("catalogo.csv"), "catalogo");
        assert_eq!(rotulo_da_origem("relatorio_q3.csv"), "relatorio_q3");
        assert_eq!(rotulo_da_origem("2025.csv"), "2025");
        assert_eq!(rotulo_da_origem("_2025.csv"), "_2025");
        assert_eq!(rotulo_da_origem("dados"), "dados");
        assert_eq!(rotulo_da_origem("a.b_2025.csv"), "a.b");
        assert_eq!(rotulo_da_origem("Área de transferência"), "Área de transferência");
    }
}
