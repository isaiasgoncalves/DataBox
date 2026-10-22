//! Os textos da tela e as regras pequenas que os montam, sem egui.

use interface::formato::{formatar_numero, plural};
use interface::icones::TipoColuna;
use tabular::csv::{Codificacao, FimDeLinha, Formato};
use tabular::numero::TipoDetectado;

/// "Lendo vendas_2025.csv…" do card de carregamento.
pub fn lendo(nome: &str) -> String {
    format!("Lendo {nome}…")
}

/// A dica dos campos de Formato com alteração pendente.
pub const DICA_FORMATO_TRAVADO: &str = "Salve ou desfaça as alterações para mudar como o arquivo é lido.";
/// A caixa do Formato.
pub const PRIMEIRA_LINHA_CABECALHO: &str = "Primeira linha é o cabeçalho";
/// A dica do Renomear (o item do menu, o F2, o botão das Propriedades) sem cabeçalho: o nome não seria gravado.
pub const DICA_SEM_CABECALHO: &str = "Ligue \"Primeira linha é o cabeçalho\" para dar nome às colunas.";
/// O toast da cola por cima e do Trazer que não mudaram nada (os valores já eram esses).
pub const NADA_MUDOU: &str = "Nada mudou: as células já tinham esses valores.";
/// O aviso de topo quando o foco volta e o arquivo mudou por fora.
pub const MUDOU_NO_DISCO: &str = "O arquivo mudou no disco.";
/// As contas da seção Coluna enquanto a tarefa passa pela coluna.
pub const CALCULANDO: &str = "Calculando…";

/// A pastilha do cabeçalho pelo tipo detectado: data `31`, texto `Aa`, inteiro `12`, decimal `,5`, booleano `✓`.
pub fn pastilha(tipo: TipoDetectado) -> TipoColuna {
    match tipo {
        TipoDetectado::Inteiro => TipoColuna::Inteiro,
        TipoDetectado::Decimal => TipoColuna::Real,
        TipoDetectado::Data => TipoColuna::Data,
        TipoDetectado::Booleano => TipoColuna::Booleano,
        TipoDetectado::Texto | TipoDetectado::Vazio => TipoColuna::Texto,
    }
}

/// O tipo em palavras ("número inteiro"): o "Tipo detectado" das Propriedades.
pub fn rotulo_do_tipo(tipo: TipoDetectado) -> &'static str {
    match tipo {
        TipoDetectado::Inteiro => "número inteiro",
        TipoDetectado::Decimal => "número decimal",
        TipoDetectado::Data => "data",
        TipoDetectado::Booleano => "booleano",
        TipoDetectado::Texto => "texto",
        TipoDetectado::Vazio => "vazia",
    }
}

/// A dica da pastilha no cabeçalho (o `title` da prancheta 7): "número inteiro (detectado)"; o texto, só "texto".
pub fn dica_do_tipo(tipo: TipoDetectado) -> String {
    match tipo {
        TipoDetectado::Texto | TipoDetectado::Vazio => "texto".to_string(),
        outro => format!("{} (detectado)", rotulo_do_tipo(outro)),
    }
}

/// Coluna de números (alinha à direita, `td.num`).
pub fn numerica(tipo: TipoDetectado) -> bool {
    matches!(tipo, TipoDetectado::Inteiro | TipoDetectado::Decimal)
}

/// O formato em pílulas na faixa (prancheta 7): o separador em palavras, a codificação e o fim de linha ("Ponto e
/// vírgula", "UTF-8", "CRLF"; a pílula põe em caixa alta).
pub fn pilulas_do_formato(f: &Formato) -> Vec<String> {
    vec![
        rotulo_do_separador(f.separador),
        codificacao_curta(f.codificacao).to_string(),
        fim_de_linha_curto(f.fim_de_linha).to_string(),
    ]
}

/// O formato no rodapé, depois da contagem: "· Ponto e vírgula · UTF-8 · CRLF" (as pílulas da antiga faixa).
pub fn formato_no_rodape(f: &Formato) -> String {
    format!("· {}", pilulas_do_formato(f).join(" · "))
}

/// "· 1 alteração não salva" / "· 3 alterações não salvas".
pub fn alteracoes_nao_salvas(n: usize) -> String {
    if n == 1 {
        "· 1 alteração não salva".to_string()
    } else {
        format!("· {} alterações não salvas", formatar_numero(n as u64))
    }
}

/// "· 2 marcadas" (e "· 1 marcada").
pub fn marcadas(n: usize) -> String {
    format!("· {}", plural(n as u64, "marcada", "marcadas"))
}

/// "· coluna valor selecionada" / "· 3 colunas selecionadas" (prancheta 16).
pub fn colunas_selecionadas(nomes: &[&str]) -> String {
    match nomes {
        [um] => format!("· coluna {um} selecionada"),
        varios => format!("· {} colunas selecionadas", varios.len()),
    }
}
/// "Salvando vendas_2025.csv…" (o aviso da gravação que passa de 0,4 s).
pub fn salvando(nome: &str) -> String {
    format!("Salvando {nome}…")
}

/// O toast da gravação.
pub fn salvo(nome: &str) -> String {
    format!("{nome} salvo.")
}

/// O Salvar quando outro programa mudou o arquivo: o núcleo regrava a partir do arquivo de origem, então não há como
/// salvar por cima (decisão 9, revista no passo 11).
pub const MUDOU_AO_SALVAR: &str = "O arquivo mudou no disco desde que foi aberto; nada foi gravado. Leia o arquivo de novo para continuar (as alterações daqui se perdem).";

/// "A seleção é grande demais para copiar (N células).".
pub fn grande_demais_para_copiar(celulas: u64) -> String {
    format!("A seleção é grande demais para copiar ({} células).", formatar_numero(celulas))
}

/// O separador em palavras (o seletor do Formato).
pub fn rotulo_do_separador(c: char) -> String {
    match c {
        ';' => "Ponto e vírgula".to_string(),
        ',' => "Vírgula".to_string(),
        '\t' => "Tabulação".to_string(),
        '|' => "Barra vertical".to_string(),
        outro => format!("Outro ({outro})"),
    }
}

/// O separador na barra de status ("Separador ;", "Separador Tab").
pub fn separador_curto(c: char) -> String {
    match c {
        '\t' => "Separador Tab".to_string(),
        outro => format!("Separador {outro}"),
    }
}

pub fn rotulo_do_decimal(c: char) -> &'static str {
    if c == '.' { "Ponto" } else { "Vírgula" }
}

/// O fim de linha na barra de status ("CRLF").
pub fn fim_de_linha_curto(f: FimDeLinha) -> &'static str {
    match f {
        FimDeLinha::Crlf => "CRLF",
        FimDeLinha::Lf => "LF",
        FimDeLinha::Cr => "CR",
    }
}

/// Os detalhes da barra de status: "CSV · UTF-8 · CRLF · Separador ;".
pub fn detalhes_do_status(f: &Formato) -> Vec<String> {
    vec![
        "CSV".to_string(),
        codificacao_curta(f.codificacao).to_string(),
        fim_de_linha_curto(f.fim_de_linha).to_string(),
        separador_curto(f.separador),
    ]
}

/// A codificação na barra de status ("UTF-8", "UTF-8 com BOM", "Windows-1252").
pub fn codificacao_curta(c: Codificacao) -> &'static str {
    c.rotulo()
}

/// Um número de uma estatística (Soma) escrito como o arquivo escreve: o decimal do arquivo e o milhar ao contrário
/// (`14.448`; `1.234,5` com decimal `,`; `1,234.5` com `.`), com até `casas` casas decimais.
pub fn numero_no_formato(valor: f64, decimal: char, casas: usize) -> String {
    let texto = format!("{:.*}", casas.min(10), valor.abs());
    let (inteiro, fracao) = texto.split_once('.').unwrap_or((texto.as_str(), ""));
    let milhar = if decimal == '.' { ',' } else { '.' };
    let mut com_milhar = String::with_capacity(inteiro.len() + inteiro.len() / 3);
    for (i, ch) in inteiro.chars().enumerate() {
        if i > 0 && (inteiro.len() - i) % 3 == 0 {
            com_milhar.push(milhar);
        }
        com_milhar.push(ch);
    }
    let fracao = fracao.trim_end_matches('0');
    let sinal = if valor < 0.0 && (inteiro.bytes().any(|b| b != b'0') || !fracao.is_empty()) { "-" } else { "" };
    if fracao.is_empty() { format!("{sinal}{com_milhar}") } else { format!("{sinal}{com_milhar}{decimal}{fracao}") }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn contagens_do_rodape_e_pilulas() {
        let f = Formato { codificacao: Codificacao::Utf8, ..Formato::padrao() };
        assert_eq!(pilulas_do_formato(&f), ["Ponto e vírgula", "UTF-8", "CRLF"]);
        let f = Formato { separador: '\t', fim_de_linha: FimDeLinha::Lf, ..Formato::padrao() };
        assert_eq!(pilulas_do_formato(&f), ["Tabulação", "UTF-8 com BOM", "LF"]);
        assert_eq!(alteracoes_nao_salvas(1), "· 1 alteração não salva");
        assert_eq!(alteracoes_nao_salvas(3), "· 3 alterações não salvas");
    }

    #[test]
    fn numeros_no_formato_do_arquivo() {
        assert_eq!(numero_no_formato(14448.0, ',', 0), "14.448");
        assert_eq!(numero_no_formato(1234.5, ',', 2), "1.234,5");
        assert_eq!(numero_no_formato(1234.5, '.', 2), "1,234.5");
        assert_eq!(numero_no_formato(-0.25, ',', 2), "-0,25");
        assert_eq!(numero_no_formato(999.0, ',', 0), "999");
    }

    #[test]
    fn tipos_em_palavras_e_pastilhas() {
        assert_eq!(dica_do_tipo(TipoDetectado::Inteiro), "número inteiro (detectado)");
        assert_eq!(dica_do_tipo(TipoDetectado::Texto), "texto");
        assert_eq!(pastilha(TipoDetectado::Decimal), TipoColuna::Real);
        assert!(numerica(TipoDetectado::Decimal) && !numerica(TipoDetectado::Data));
    }
}
