//! A estrutura de um JSON: uma arena de nós com a posição de cada valor no texto, montada sem recursão.
//!
//! Os nós ficam num `Vec` na **ordem do documento** (cada pai antes dos seus filhos), e cada um sabe o pai, o primeiro
//! filho e o próximo irmão. Por isso a árvore não tem ponteiros nem recursão (nem para montar, nem para percorrer,
//! nem para soltar da memória: 100.000 níveis não estouram a pilha), e achar o nó sob um byte é uma busca binária.
//!
//! Cada nó custa uns 64 bytes: uma lista de 4 milhões de números (8 MiB de `1,1,1,…`, o pior caso do
//! [`crate::limites::LIMITE_ESTRUTURA`]) ocupa uns 250 MB. O JSON real de 8 MiB tem bem menos nós.

use std::ops::Range;

/// Quantos filhos de um objeto ou lista a estrutura mostra de uma vez (o resto vira "mais 1.800 itens").
pub const FILHOS_POR_PAGINA: usize = 200;

/// Marca de "nenhum nó" nos encadeamentos da arena.
pub(super) const NENHUM: u32 = u32::MAX;

/// O que um valor é. `Numero` é o que tem ponto ou expoente (`1.5`, `1e5`); `Inteiro`, só dígitos (`42`, `-7`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tipo {
    Objeto,
    Lista,
    Texto,
    Numero,
    Inteiro,
    Booleano,
    Nulo,
}

impl Tipo {
    /// Objeto ou lista.
    pub fn e_container(self) -> bool {
        matches!(self, Tipo::Objeto | Tipo::Lista)
    }
}

/// O identificador de um nó numa [`Arvore`]: o índice dele na arena (a ordem do documento).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoId(pub(super) u32);

impl NoId {
    /// A posição do nó na ordem do documento (0 é a raiz). Serve de identidade estável enquanto a árvore existir.
    pub fn indice(self) -> usize {
        self.0 as usize
    }
}

/// Um valor do documento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct No {
    pub tipo: Tipo,
    /// O byte onde o valor começa (`{`, `[`, a aspa, o primeiro dígito ou letra).
    pub inicio: usize,
    /// O byte logo depois do fim do valor (depois do `}`, do `]`, da aspa de fechamento…).
    pub fim: usize,
    /// Num objeto, onde está o nome da chave deste valor, **com** as aspas.
    pub chave: Option<Range<usize>>,
    /// Quantos filhos (chaves de um objeto, itens de uma lista); 0 nos demais.
    pub filhos: u32,
    pub(super) pai: u32,
    pub(super) primeiro: u32,
    pub(super) irmao: u32,
    /// A posição entre os irmãos (a chave de uma lista).
    pub(super) ordem: u32,
}

impl No {
    pub(super) fn novo(tipo: Tipo, inicio: usize, fim: usize, chave: Option<Range<usize>>, pai: u32, ordem: u32) -> No {
        No { tipo, inicio, fim, chave, filhos: 0, pai, primeiro: NENHUM, irmao: NENHUM, ordem }
    }

    /// Onde o nó começa para quem aponta um byte: a chave, quando tem (clicar na chave é clicar no item).
    fn comeco(&self) -> usize {
        self.chave.as_ref().map_or(self.inicio, |c| c.start)
    }
}

/// A estrutura de um documento JSON válido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arvore {
    nos: Vec<No>,
}

/// Os filhos de um nó, na ordem do documento.
pub struct Filhos<'a> {
    arvore: &'a Arvore,
    proximo: u32,
}

impl Iterator for Filhos<'_> {
    type Item = NoId;

    fn next(&mut self) -> Option<NoId> {
        if self.proximo == NENHUM {
            return None;
        }
        let id = NoId(self.proximo);
        self.proximo = self.arvore.no(id).irmao;
        Some(id)
    }
}

/// Uma página dos filhos de um nó (no máximo [`FILHOS_POR_PAGINA`]) e quantos ficaram para depois.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaginaDeFilhos {
    pub nos: Vec<NoId>,
    /// Quantos filhos há depois desta página (a linha "mais 1.800 itens").
    pub restantes: usize,
}

impl Arvore {
    /// A arena montada pelo analisador: a raiz é o primeiro nó.
    pub(super) fn de_nos(nos: Vec<No>) -> Arvore {
        debug_assert!(!nos.is_empty());
        Arvore { nos }
    }

    /// O valor do documento inteiro.
    pub fn raiz(&self) -> NoId {
        NoId(0)
    }

    /// Quantos nós (valores) o documento tem.
    pub fn total(&self) -> usize {
        self.nos.len()
    }

    pub fn no(&self, id: NoId) -> &No {
        &self.nos[id.0 as usize]
    }

    /// O pai do nó (`None` na raiz).
    pub fn pai(&self, id: NoId) -> Option<NoId> {
        let pai = self.no(id).pai;
        (pai != NENHUM).then_some(NoId(pai))
    }

    /// Os filhos do nó, na ordem do documento.
    pub fn filhos(&self, id: NoId) -> Filhos<'_> {
        Filhos { arvore: self, proximo: self.no(id).primeiro }
    }

    /// A posição do nó entre os irmãos (de 0): o índice de um item de lista.
    pub fn ordem(&self, id: NoId) -> usize {
        self.no(id).ordem as usize
    }

    /// Os nós do caminho do documento até o byte, do mais externo (a raiz) ao mais interno: o que a migalha da faixa
    /// mostra (`conexoes › 0 › tempo_limite`, sem a raiz). Um byte na chave de um item vale o item; no começo ou no
    /// fim de um valor, vale o valor (o cursor logo depois do `30` está no `30`); entre os filhos de um objeto,
    /// vale o objeto. Fora do documento (nos espaços antes ou depois dele), nenhum nó: caminho vazio.
    ///
    /// É uma busca binária pelos começos dos nós (estão em ordem) e uma subida pelos pais até achar quem abrange o byte.
    pub fn caminho_em(&self, byte: usize) -> Vec<NoId> {
        let ate = self.nos.partition_point(|n| n.comeco() <= byte);
        let Some(ultimo) = ate.checked_sub(1) else {
            return Vec::new();
        };
        let mut atual = NoId(u32::try_from(ultimo).unwrap_or(NENHUM));
        while byte > self.no(atual).fim {
            match self.pai(atual) {
                Some(pai) => atual = pai,
                None => return Vec::new(),
            }
        }
        let mut caminho = vec![atual];
        while let Some(pai) = self.pai(atual) {
            caminho.push(pai);
            atual = pai;
        }
        caminho.reverse();
        caminho
    }

    /// O nome do nó na estrutura: a chave (com os escapes desfeitos), o índice do item numa lista ("0", "1"…) ou
    /// "raiz" no documento inteiro. `texto` é o mesmo que foi analisado.
    pub fn nome(&self, id: NoId, texto: &str) -> String {
        let no = self.no(id);
        match &no.chave {
            Some(chave) => {
                let dentro = texto.get(chave.start + 1..chave.end.saturating_sub(1)).unwrap_or_default();
                desescapar(dentro)
            }
            None if no.pai == NENHUM => "raiz".to_string(),
            None => no.ordem.to_string(),
        }
    }

    /// O texto do valor como está escrito no documento (`30`, `"Produção"`, `true`, ou o objeto inteiro).
    pub fn trecho<'t>(&self, id: NoId, texto: &'t str) -> &'t str {
        let no = self.no(id);
        texto.get(no.inicio..no.fim).unwrap_or_default()
    }

    /// O valor de um nó de texto, sem as aspas e com os escapes desfeitos (`ç` vira `ç`). `None` nos outros tipos.
    pub fn valor_de_texto(&self, id: NoId, texto: &str) -> Option<String> {
        let no = self.no(id);
        if no.tipo != Tipo::Texto {
            return None;
        }
        texto.get(no.inicio + 1..no.fim.saturating_sub(1)).map(desescapar)
    }

    /// Uma página dos filhos do nó, a partir do `a_partir`-ésimo: no máximo [`FILHOS_POR_PAGINA`] e quantos sobram. A
    /// estrutura mostra a primeira página e, ao clique em "mais 1.800 itens", a seguinte.
    pub fn pagina_de_filhos(&self, id: NoId, a_partir: usize) -> PaginaDeFilhos {
        let total = self.no(id).filhos as usize;
        let nos: Vec<NoId> = self.filhos(id).skip(a_partir).take(FILHOS_POR_PAGINA).collect();
        let restantes = total.saturating_sub(a_partir + nos.len());
        PaginaDeFilhos { nos, restantes }
    }

    /// A contagem à direita do nó: "5 chaves" ("1 chave") num objeto, "2 itens" ("1 item") numa lista; `None` nos
    /// valores simples.
    pub fn contagem(&self, id: NoId) -> Option<String> {
        let no = self.no(id);
        let n = no.filhos as usize;
        match no.tipo {
            Tipo::Objeto => Some(format!("{n} {}", if n == 1 { "chave" } else { "chaves" })),
            Tipo::Lista => Some(format!("{n} {}", if n == 1 { "item" } else { "itens" })),
            _ => None,
        }
    }
}

/// "mais 1.800 itens" ("mais 1 item"): a linha que mostra a página seguinte dos filhos.
pub fn mais_itens_em_texto(restantes: usize) -> String {
    format!("mais {} {}", milhares(restantes), if restantes == 1 { "item" } else { "itens" })
}

/// O número com ponto a cada três dígitos: 1800 → "1.800".
pub fn milhares(n: usize) -> String {
    let digitos = n.to_string();
    let mut saida = String::with_capacity(digitos.len() + digitos.len() / 3);
    for (i, c) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i).is_multiple_of(3) {
            saida.push('.');
        }
        saida.push(c);
    }
    saida
}

/// O conteúdo de uma cadeia JSON (o que fica entre as aspas) com os escapes desfeitos: `\n`, `\"`, `\\`, `\/`, `\b`,
/// `\f`, `\r`, `\t` e `\uXXXX` (com o par substituto de um caractere fora do plano básico; um substituto solto vira
/// U+FFFD). Um escape malformado, que o analisador já recusou, fica como está.
pub fn desescapar(interno: &str) -> String {
    if !interno.contains('\\') {
        return interno.to_string();
    }
    let mut saida = String::with_capacity(interno.len());
    let mut chars = interno.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            saida.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => saida.push('\n'),
            Some('t') => saida.push('\t'),
            Some('r') => saida.push('\r'),
            Some('b') => saida.push('\u{8}'),
            Some('f') => saida.push('\u{c}'),
            Some(c @ ('"' | '\\' | '/')) => saida.push(c),
            Some('u') => match quatro_digitos(&mut chars) {
                Some(alto @ 0xD800..=0xDBFF) => {
                    // par substituto: o `\uXXXX` seguinte tem de ser o baixo
                    let mut copia = chars.clone();
                    let baixo = (copia.next() == Some('\\') && copia.next() == Some('u'))
                        .then(|| quatro_digitos(&mut copia))
                        .flatten()
                        .filter(|b| (0xDC00..=0xDFFF).contains(b));
                    match baixo {
                        Some(baixo) => {
                            chars = copia;
                            let ponto = 0x10000 + ((alto - 0xD800) << 10) + (baixo - 0xDC00);
                            saida.push(char::from_u32(ponto).unwrap_or('\u{FFFD}'));
                        }
                        None => saida.push('\u{FFFD}'),
                    }
                }
                Some(0xDC00..=0xDFFF) => saida.push('\u{FFFD}'),
                Some(ponto) => saida.push(char::from_u32(ponto).unwrap_or('\u{FFFD}')),
                None => saida.push_str("\\u"),
            },
            Some(outro) => {
                saida.push('\\');
                saida.push(outro);
            }
            None => saida.push('\\'),
        }
    }
    saida
}

/// Lê quatro dígitos hexadecimais; se não forem quatro, não consome nada.
fn quatro_digitos(chars: &mut std::str::Chars<'_>) -> Option<u32> {
    let mut copia = chars.clone();
    let mut valor = 0u32;
    for _ in 0..4 {
        valor = valor * 16 + copia.next()?.to_digit(16)?;
    }
    *chars = copia;
    Some(valor)
}
