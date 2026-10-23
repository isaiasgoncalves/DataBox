//! As regras puras da aba Dados: operadores e rótulos dos filtros, filtros completos,
//! a chave de uma busca, as regras de uma coluna numa frase, títulos da janela da linha, os textos do hint e do
//! rodapé e a repartição da largura da grade. Nada aqui desenha nem chama o serviço.

use banco::{ColunaInfo, Operador};
use comum::{Afinidade, Celula, Chave, Valor};
use estado::{FiltroDigitado, POR_PAGINA_OPCOES};
use interface::formato::{formatar_numero, plural, real_js};
use interface::grade::texto_celula;
use interface::pecas::rotulos::TipoPill;

/// Os 14 operadores de filtro, na ordem e com os rótulos do menu.
pub const OPERADORES: [(Operador, &str); 14] = [
    (Operador::Contem, "contém"),
    (Operador::NaoContem, "não contém"),
    (Operador::Igual, "igual a"),
    (Operador::Diferente, "diferente de"),
    (Operador::Maior, "maior que"),
    (Operador::MaiorIgual, "maior ou igual"),
    (Operador::Menor, "menor que"),
    (Operador::MenorIgual, "menor ou igual"),
    (Operador::Comeca, "começa com"),
    (Operador::Termina, "termina com"),
    (Operador::Nulo, "é nulo"),
    (Operador::NaoNulo, "não é nulo"),
    (Operador::Vazio, "é vazio"),
    (Operador::NaoVazio, "não é vazio"),
];

/// Segundos que a busca e o valor do filtro esperam depois da última tecla (`ESPERA_DIGITACAO`).
pub const ESPERA_DIGITACAO: f64 = 0.3;
/// Texto acima disto (ou com quebra de linha) ganha uma área de texto na janela (`TEXTO_CURTO`).
pub const TEXTO_CURTO: usize = 80;
/// Ordenar mais lento que isto sugere um índice (`ORDENAR_LENTO_MS`).
pub const ORDENAR_LENTO_MS: f64 = 1500.0;
/// Buscar em todas as colunas mais lento que isto sugere um filtro por coluna (`BUSCA_LENTA_MS`).
pub const BUSCA_LENTA_MS: f64 = 3000.0;

/// O aviso da janela de uma linha sem chave (PK nula).
pub const AVISO_SEM_CHAVE: &str = "Esta linha não tem chave: a chave primária dela está vazia (NULL), então não há \
                                   como apontar para ela ao gravar. Aqui ela é só leitura; para mudá-la, use a \
                                   Consulta SQL.";
pub use interface::pecas::filtros::{CONTANDO, LINHAS_NAO_CONTADAS};
pub const LENDO: &str = "Lendo as linhas…";
pub const VAZIO_FILTRANDO: &str = "Nenhuma linha corresponde à busca ou aos filtros.";
/// O "+ Nova linha" do card saiu (etapa 3): a linha nova é o Nova linha da barra de ferramentas.
pub const VAZIO_EDITAVEL: &str = "Nenhuma linha. Use Nova linha, na barra de ferramentas, para incluir a primeira.";
pub const VAZIO_VISAO: &str = "A visão não devolveu nenhuma linha.";
pub const VAZIO_TABELA: &str = "Nenhuma linha nesta tabela.";
pub const DICA_BUSCA_LENTA: &str = "A busca percorre todas as colunas; um filtro por coluna costuma ser mais rápido.";

/// Um filtro como está na barra (pode estar incompleto: valor ainda vazio).
#[derive(Debug, Clone, PartialEq)]
pub struct LinhaFiltro {
    /// Identidade estável da linha na tela (os ids dos campos não mudam ao remover outra).
    pub id: u64,
    pub coluna: String,
    pub op: Operador,
    pub valor: String,
}

/// Os filtros que já dizem algo: operador sem valor, ou com o valor digitado (`filtrosCompletos`). O valor dos
/// operadores sem valor vai vazio.
pub fn filtros_completos(filtros: &[LinhaFiltro]) -> Vec<FiltroDigitado> {
    filtros
        .iter()
        .filter(|f| f.op.sem_valor() || !f.valor.is_empty())
        .map(|f| FiltroDigitado {
            coluna: f.coluna.clone(),
            op: f.op,
            valor: if f.op.sem_valor() { String::new() } else { f.valor.clone() },
        })
        .collect()
}

/// A chave do conjunto de linhas (busca + filtros completos), que identifica a contagem dele (`chaveFiltro`).
pub fn chave_filtro(busca: &str, filtros: &[FiltroDigitado]) -> String {
    let mut s = String::with_capacity(busca.len() + 16 * filtros.len() + 8);
    s.push_str(&format!("{busca:?}"));
    for f in filtros {
        s.push_str(&format!("|{:?}\u{1f}{}\u{1f}{:?}", f.coluna, f.op.como_str(), f.valor));
    }
    s
}

/// Texto que identifica uma chave: igual para chaves iguais.
pub fn chave_texto(chave: &Chave) -> String {
    format!("{chave:?}")
}

/// O texto de um valor exato como a célula escreveria (o `textoCelula` de um valor da chave).
pub fn texto_valor(v: &Valor) -> String {
    match v {
        Valor::Nulo | Valor::Blob(_) => String::new(),
        Valor::Inteiro(i) => i.to_string(),
        Valor::Real(r) => real_js(*r),
        Valor::Texto(s) => s.clone(),
        Valor::TextoBruto(b) => String::from_utf8_lossy(b).into_owned(),
    }
}

/// O título da janela da linha (`tituloLinha`): "Linha 12", "Linha P003" (chave primária de uma coluna só, como a
/// prancheta 12), "Linha de pedido_id 7 · produto P003", ou a posição da
/// linha na busca quando não há chave.
pub fn titulo_linha(chave: Option<&Chave>, posicao: u64) -> String {
    match chave {
        Some(Chave::Rowid(r)) => format!("Linha {r}"),
        Some(Chave::Pk(partes)) if partes.len() == 1 => format!("Linha {}", texto_valor(&partes[0].1)),
        Some(Chave::Pk(partes)) if !partes.is_empty() => {
            let partes: Vec<String> = partes.iter().map(|(k, v)| format!("{k} {}", texto_valor(v))).collect();
            format!("Linha de {}", partes.join(" · "))
        }
        _ => format!("Linha {}", formatar_numero(posicao)),
    }
}

/// A linha na dica de um verbo: o título sem o "Linha " / "Linha de " do começo ("3", "P003").
pub fn descricao_linha(titulo: &str) -> &str {
    titulo.strip_prefix("Linha de ").or_else(|| titulo.strip_prefix("Linha ")).unwrap_or(titulo)
}

/// A linha na frase de exclusão da ficha ("Excluir a linha {} de produtos?"): "12" (rowid), "com codigo P003" (a
/// chave primária, com os nomes das colunas: "com pedido_id 7 · produto P003").
pub fn descricao_exclusao(chave: &Chave) -> String {
    match chave {
        Chave::Pk(partes) if !partes.is_empty() => {
            let partes: Vec<String> = partes.iter().map(|(k, v)| format!("{k} {}", texto_valor(v))).collect();
            format!("com {}", partes.join(" · "))
        }
        _ => descricao_linha(&titulo_linha(Some(chave), 0)).to_string(),
    }
}

/// As regras da coluna numa frase curta (`regrasColuna`): "INTEGER · chave primária · autoincremento".
pub fn regras_coluna(col: &ColunaInfo) -> String {
    let mut partes: Vec<String> = vec![if col.tipo.is_empty() { "sem tipo".to_string() } else { col.tipo.clone() }];
    if col.pk > 0 {
        partes.push("chave primária".into());
    }
    if col.autoincremento || col.rowid_alias {
        partes.push("autoincremento".into());
    }
    if col.nao_nulo && col.pk == 0 {
        partes.push("não aceita nulo".into());
    }
    if col.unica {
        partes.push("único".into());
    }
    if let Some(p) = &col.padrao {
        partes.push(format!("padrão {p}"));
    }
    if col.gerada {
        partes.push(match col.oculta {
            2 => "gerada (VIRTUAL)".into(),
            3 => "gerada (STORED)".into(),
            _ => "gerada".into(),
        });
    }
    if let Some(fk) = &col.fk {
        match &fk.coluna {
            Some(c) => partes.push(format!("→ {}.{c}", fk.tabela)),
            None => partes.push(format!("→ {}", fk.tabela)),
        }
    }
    partes.join(" · ")
}

/// Extensão provável de um BLOB pelos primeiros bytes (só para sugerir o nome do arquivo).
pub fn extensao_do_blob(inicio: &[u8]) -> &'static str {
    const ASSINATURAS: [(&[u8], &str); 8] = [
        (&[0x89, 0x50, 0x4e, 0x47], "png"),
        (&[0xff, 0xd8, 0xff], "jpg"),
        (&[0x47, 0x49, 0x46, 0x38], "gif"),
        (&[0x25, 0x50, 0x44, 0x46], "pdf"),
        (&[0x50, 0x4b, 0x03, 0x04], "zip"),
        (&[0x52, 0x49, 0x46, 0x46], "webp"),
        (&[0x42, 0x4d], "bmp"),
        (b"SQLite format", "db"),
    ];
    ASSINATURAS.iter().find(|(prefixo, _)| inicio.starts_with(prefixo)).map_or("bin", |(_, ext)| ext)
}

/// Afinidades em que a grade e a janela tratam a coluna como número.
pub fn afinidade_numerica(a: Afinidade) -> bool {
    a.numerica()
}

/// O rótulo do número da linha (o abridor): o rowid, ou "abrir" (`rotuloLinha`).
pub fn rotulo_linha(chave: Option<&Chave>) -> String {
    match chave {
        Some(Chave::Rowid(r)) => r.to_string(),
        _ => "abrir".to_string(),
    }
}

/// Texto que a célula tem uma quebra de linha (não edita no lugar).
pub fn tem_quebra(c: &Celula) -> bool {
    matches!(c, Celula::Texto(s) if s.contains(['\r', '\n']))
}

/// O que o campo de edição mostra ao começar (o `textoCelula`).
pub fn texto_edicao(c: &Celula) -> String {
    texto_celula(c)
}

/// A contagem das linhas que passam ("41 de 60 linhas", "60 linhas", a da faixa até a 0.2.3; hoje, a do retrato dos
/// testes) e a do rodapé ("Linhas 1–100 de 5.000"): as das grades, iguais no CSV.
pub use interface::formato::{contagem_filtrada as texto_contagem_faixa, contagem_linhas as texto_contagem_linhas};

/// O rótulo do operador ("igual a"), o da etiqueta e do select.
pub fn rotulo_operador(op: Operador) -> &'static str {
    OPERADORES.iter().find(|(o, _)| *o == op).map_or("", |(_, r)| r)
}

/// A dica do Filtrar da barra: "Filtrar", "Filtrar (1 condição)", "Filtrar (2 condições)".
pub fn dica_filtrar(condicoes: usize) -> String {
    match condicoes {
        0 => "Filtrar".to_string(),
        n => format!("Filtrar ({})", plural(n, "condição", "condições")),
    }
}

/// A linha na dica de um verbo: "3", "P003" (o título da ficha sem o "Linha ").
fn linha_da_dica(chave: &Chave) -> String {
    descricao_linha(&titulo_linha(Some(chave), 0)).to_string()
}

/// A dica do Duplicar: "Duplicar a linha 3"; com várias marcadas, desligado, "Duplicar (marque uma
/// linha só)".
pub fn dica_duplicar(alvo: Option<&Chave>, marcadas: usize) -> String {
    match alvo {
        Some(chave) => format!("Duplicar a linha {}", linha_da_dica(chave)),
        None if marcadas > 1 => "Duplicar (marque uma linha só)".to_string(),
        None => "Duplicar".to_string(),
    }
}

/// A dica do Excluir: "Excluir a linha 3" (a marcada, ou a da célula ativa), "Excluir as 2 linhas marcadas".
pub fn dica_excluir(alvo: &[Chave], marcadas: usize) -> String {
    match alvo {
        [chave] => format!("Excluir a linha {}", linha_da_dica(chave)),
        _ if marcadas > 1 => format!("Excluir as {} linhas marcadas", formatar_numero(marcadas as u64)),
        _ => "Excluir (marque linhas ou selecione uma célula)".to_string(),
    }
}

/// As regras de uma coluna em pills (a seção "Coluna" das Propriedades): chave primária (na cor de destaque),
/// autoincremento, não aceita nulo, único, gerada (apagada) e a referência ("→ pedidos.id").
pub fn regras_em_pills(col: &ColunaInfo) -> Vec<(String, TipoPill)> {
    let mut v = Vec::new();
    if col.pk > 0 {
        v.push(("chave primária".to_string(), TipoPill::Acc));
    }
    if col.autoincremento {
        v.push(("autoincremento".to_string(), TipoPill::Neutra));
    }
    if col.nao_nulo && col.pk == 0 {
        v.push(("não aceita nulo".to_string(), TipoPill::Neutra));
    }
    if col.unica {
        v.push(("único".to_string(), TipoPill::Neutra));
    }
    if col.gerada {
        v.push(("gerada".to_string(), TipoPill::Mute));
    }
    if let Some(fk) = &col.fk {
        let alvo = match &fk.coluna {
            Some(c) => format!("→ {}.{c}", fk.tabela),
            None => format!("→ {}", fk.tabela),
        };
        v.push((alvo, TipoPill::Neutra));
    }
    v
}

/// O intervalo das linhas da página no rodapé: (primeira, última), com primeira 0 quando não há linhas.
pub fn intervalo(pagina: u64, por_pagina: u64, n_linhas: usize, total: Option<u64>) -> (u64, u64) {
    let inicio = if n_linhas > 0 { (pagina.saturating_sub(1)).saturating_mul(por_pagina) + 1 } else { 0 };
    let fim = match total {
        None => (inicio + n_linhas as u64).saturating_sub(1),
        Some(t) => t.min(pagina.saturating_mul(por_pagina)),
    };
    (inicio, fim)
}

/// O começo do texto do rodapé antes do "contando…" (total ainda não conhecido).
pub fn texto_contagem_sem_total(inicio: u64, fim: u64) -> String {
    if inicio > 0 {
        format!("Linhas {}–{} de ", formatar_numero(inicio), formatar_numero(fim))
    } else {
        "Linhas de ".to_string()
    }
}

/// "Página 2 de 50" / "Página 2" (sem o total).
pub fn texto_pagina(pagina: u64, paginas: Option<u64>) -> String {
    match paginas {
        Some(n) => format!("Página {} de {}", formatar_numero(pagina), formatar_numero(n)),
        None => format!("Página {}", formatar_numero(pagina)),
    }
}

/// Os tamanhos de página do select: os quatro de sempre e, se o endereço pediu outro, ele também, em ordem.
pub fn tamanhos_por_pagina(atual: u64) -> Vec<u64> {
    let mut v = POR_PAGINA_OPCOES.to_vec();
    if !v.contains(&atual) {
        v.push(atual);
        v.sort_unstable();
    }
    v
}

/// Número de páginas para um total (no mínimo 1).
pub fn paginas_de(total: u64, por_pagina: u64) -> u64 {
    total.div_ceil(por_pagina.max(1)).max(1)
}

/// Reparte a largura que sobra entre as colunas `a_partir_de..` na proporção da largura natural de cada uma
/// (a tabela `width:max-content; min-width:100%` da grade: o navegador estica as colunas automáticas na
/// proporção do conteúdo). Sem sobra, ficam as naturais. Grava em `saida` (sem alocar quando já tem o tamanho).
pub fn distribuir_larguras(naturais: &[f32], a_partir_de: usize, disponivel: f32, saida: &mut Vec<f32>) {
    saida.clear();
    saida.extend_from_slice(naturais);
    let soma: f32 = naturais.iter().sum();
    let soma_auto: f32 = naturais.iter().skip(a_partir_de).sum();
    let sobra = disponivel - soma;
    if sobra <= 0.5 || soma_auto <= 0.0 {
        return;
    }
    for l in saida.iter_mut().skip(a_partir_de) {
        // centésimos para baixo: a soma nunca passa do disponível (nada de rolagem lateral de 0,001px)
        *l = ((*l + sobra * (*l / soma_auto)) * 100.0).floor() / 100.0;
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn filtro(coluna: &str, op: Operador, valor: &str) -> LinhaFiltro {
        LinhaFiltro { id: 0, coluna: coluna.into(), op, valor: valor.into() }
    }

    #[test]
    fn operadores_tem_os_rotulos_de_la() {
        assert_eq!(OPERADORES.len(), 14);
        assert_eq!(OPERADORES.map(|(o, _)| o), Operador::TODOS);
        assert_eq!(OPERADORES[0].1, "contém");
        assert_eq!(OPERADORES[13].1, "não é vazio");
    }

    #[test]
    fn filtros_completos_deixa_de_fora_o_valor_vazio() {
        let filtros = [
            filtro("ativo", Operador::Igual, "1"),
            filtro("nome", Operador::Contem, ""),
            filtro("limite", Operador::NaoNulo, "lixo"),
        ];
        let c = filtros_completos(&filtros);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].valor, "1");
        assert_eq!(c[1].op, Operador::NaoNulo);
        assert_eq!(c[1].valor, "", "operador sem valor vai sem valor");
        assert_ne!(chave_filtro("ana", &c), chave_filtro("ana", &c[..1]));
        assert_ne!(chave_filtro("", &[]), chave_filtro("a", &[]));
    }

    #[test]
    fn titulo_da_linha_pela_chave() {
        assert_eq!(titulo_linha(Some(&Chave::Rowid(12)), 3), "Linha 12");
        let pk =
            Chave::Pk(vec![("pedido_id".into(), Valor::Inteiro(7)), ("produto".into(), Valor::Texto("P003".into()))]);
        assert_eq!(titulo_linha(Some(&pk), 3), "Linha de pedido_id 7 · produto P003");
        assert_eq!(titulo_linha(None, 1234), "Linha 1.234");
        assert_eq!(descricao_linha("Linha de codigo P003"), "codigo P003");
        assert_eq!(descricao_linha("Linha 2"), "2");
    }

    #[test]
    fn regras_da_coluna_numa_frase() {
        let mut c = ColunaInfo {
            cid: 0,
            nome: "id".into(),
            tipo: "INTEGER".into(),
            afinidade: Afinidade::Integer,
            nao_nulo: false,
            padrao: None,
            pk: 1,
            oculta: 0,
            gerada: false,
            autoincremento: true,
            rowid_alias: true,
            unica: false,
            fk: None,
        };
        assert_eq!(regras_coluna(&c), "INTEGER · chave primária · autoincremento");
        c.pk = 0;
        c.autoincremento = false;
        c.rowid_alias = false;
        c.tipo = "TEXT".into();
        c.nao_nulo = true;
        c.padrao = Some("'aberto'".into());
        assert_eq!(regras_coluna(&c), "TEXT · não aceita nulo · padrão 'aberto'");
        c.tipo = String::new();
        c.nao_nulo = false;
        c.padrao = None;
        c.gerada = true;
        c.fk = Some(banco::FkColuna {
            tabela: "pedidos".into(),
            coluna: Some("id".into()),
            ao_excluir: "NO ACTION".into(),
            ao_atualizar: "NO ACTION".into(),
        });
        assert_eq!(regras_coluna(&c), "sem tipo · gerada · → pedidos.id");
        c.oculta = 2;
        assert!(regras_coluna(&c).contains("gerada (VIRTUAL)"));
        c.oculta = 3;
        assert!(regras_coluna(&c).contains("gerada (STORED)"));
    }

    #[test]
    fn extensao_pelos_primeiros_bytes() {
        assert_eq!(extensao_do_blob(&[0x89, 0x50, 0x4e, 0x47, 0x0d]), "png");
        assert_eq!(extensao_do_blob(b"SQLite format 3\0"), "db");
        assert_eq!(extensao_do_blob(&[1, 2, 3]), "bin");
        assert_eq!(extensao_do_blob(&[]), "bin");
    }

    #[test]
    fn textos_da_faixa_e_do_rodape() {
        // a faixa (D1, opção B): "41 de 60 linhas" com busca ou filtro, "60 linhas" sem
        assert_eq!(texto_contagem_faixa(41, Some(60), true), "41 de 60 linhas");
        assert_eq!(texto_contagem_faixa(60, Some(60), false), "60 linhas");
        assert_eq!(texto_contagem_faixa(1, Some(1), true), "1 de 1 linha");
        assert_eq!(texto_contagem_faixa(2500, None, true), "2.500 linhas", "sem o total da tabela, só as achadas");
        assert_eq!(texto_contagem_faixa(0, Some(5000), true), "0 de 5.000 linhas");
        assert_eq!(intervalo(2, 100, 100, Some(5000)), (101, 200));
        assert_eq!(intervalo(1, 100, 60, Some(60)), (1, 60));
        assert_eq!(intervalo(2, 100, 100, None), (101, 200));
        assert_eq!(intervalo(1, 100, 0, Some(0)), (0, 0));
        assert_eq!(texto_contagem_linhas(101, 200, 5000), "Linhas 101–200 de 5.000");
        assert_eq!(texto_contagem_linhas(0, 0, 0), "Nenhuma linha");
        assert_eq!(texto_contagem_sem_total(1, 100), "Linhas 1–100 de ");
        assert_eq!(texto_pagina(2, Some(50)), "Página 2 de 50");
        assert_eq!(texto_pagina(2, None), "Página 2");
        assert_eq!(tamanhos_por_pagina(100), vec![50, 100, 250, 500]);
        assert_eq!(tamanhos_por_pagina(20), vec![20, 50, 100, 250, 500]);
        assert_eq!(paginas_de(5000, 100), 50);
        assert_eq!(paginas_de(0, 100), 1);
        assert_eq!(paginas_de(1250, 100), 13);
    }

    #[test]
    fn sobra_de_largura_vai_para_as_colunas_de_dados() {
        let mut saida = Vec::new();
        // a coluna da caixa de marcar: `.grade td.selc{width:32px}`
        distribuir_larguras(&[32.0, 100.0, 300.0], 1, 832.0, &mut saida);
        assert_eq!(saida[0], 32.0, "a caixa de marcar não estica");
        assert!((saida[1] - 200.0).abs() < 0.02 && (saida[2] - 600.0).abs() < 0.02, "{saida:?}");
        assert!(saida.iter().sum::<f32>() <= 832.0);
        distribuir_larguras(&[32.0, 100.0, 300.0], 1, 300.0, &mut saida);
        assert_eq!(saida, vec![32.0, 100.0, 300.0], "sem sobra, as naturais (a grade rola)");
    }

    #[test]
    fn dicas_dos_verbos_dizem_o_alvo() {
        assert_eq!(rotulo_operador(Operador::Igual), "igual a");
        assert_eq!(rotulo_operador(Operador::NaoNulo), "não é nulo");
        assert_eq!(dica_filtrar(0), "Filtrar");
        assert_eq!(dica_filtrar(1), "Filtrar (1 condição)");
        assert_eq!(dica_filtrar(2), "Filtrar (2 condições)");
        let tres = Chave::Rowid(3);
        let p003 = Chave::Pk(vec![("codigo".into(), Valor::Texto("P003".into()))]);
        assert_eq!(dica_duplicar(Some(&tres), 0), "Duplicar a linha 3");
        assert_eq!(dica_duplicar(Some(&p003), 1), "Duplicar a linha P003");
        // chave primária de uma coluna só: o valor, como o título da prancheta 12
        assert_eq!(titulo_linha(Some(&p003), 3), "Linha P003");
        // a frase de exclusão da ficha diz a coluna da chave: "Excluir a linha com codigo P003 de produtos?"
        assert_eq!(descricao_exclusao(&p003), "com codigo P003");
        assert_eq!(descricao_exclusao(&tres), "3");
        let composta =
            Chave::Pk(vec![("pedido_id".into(), Valor::Inteiro(7)), ("produto".into(), Valor::Texto("P003".into()))]);
        assert_eq!(descricao_exclusao(&composta), "com pedido_id 7 · produto P003");
        assert_eq!(dica_duplicar(None, 2), "Duplicar (marque uma linha só)");
        assert_eq!(dica_duplicar(None, 0), "Duplicar");
        assert_eq!(dica_excluir(std::slice::from_ref(&tres), 0), "Excluir a linha 3");
        assert_eq!(dica_excluir(&[tres.clone(), p003], 2), "Excluir as 2 linhas marcadas");
        assert_eq!(dica_excluir(&[], 0), "Excluir (marque linhas ou selecione uma célula)");
    }

    #[test]
    fn regras_da_coluna_em_pills() {
        let mut c = ColunaInfo {
            cid: 0,
            nome: "id".into(),
            tipo: "INTEGER".into(),
            afinidade: Afinidade::Integer,
            nao_nulo: false,
            padrao: None,
            pk: 1,
            oculta: 0,
            gerada: false,
            autoincremento: true,
            rowid_alias: true,
            unica: false,
            fk: None,
        };
        let textos = |c: &ColunaInfo| regras_em_pills(c).into_iter().map(|(t, _)| t).collect::<Vec<_>>();
        assert_eq!(textos(&c), ["chave primária", "autoincremento"]);
        assert_eq!(regras_em_pills(&c)[0].1, TipoPill::Acc);
        c.pk = 0;
        c.autoincremento = false;
        c.nao_nulo = true;
        c.unica = true;
        assert_eq!(textos(&c), ["não aceita nulo", "único"]);
        c.nao_nulo = false;
        c.unica = false;
        assert!(regras_em_pills(&c).is_empty());
    }
}
