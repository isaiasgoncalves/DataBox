//! Os idiomas do DataBox, no estilo do gettext: **a chave é o próprio texto em português**. O código continua
//! escrito em português, e cada idioma é um catálogo `.po` em `crates/idioma/idiomas/`, embutido no executável.
//! Sem catálogo, ou sem a chave nele, o texto sai em português.
//!
//! # As macros
//!
//! ```
//! use idioma::{t, tn, t_estatico};
//!
//! # idioma::com_idioma(idioma::PORTUGUES, || {
//! // um texto fixo: Cow<'static, str>, emprestado do catálogo (ou da própria chave), sem alocar
//! assert_eq!(t!("Abrir pasta"), "Abrir pasta");
//!
//! // com argumentos nomeados: cada {nome} vira o Display do valor
//! let tabela = "vendas";
//! assert_eq!(t!("Gravado em {tabela} (linha {linha})", tabela = tabela, linha = 12), "Gravado em vendas (linha 12)");
//!
//! // o plural: o primeiro argumento é a contagem (qualquer inteiro), que entra no {n} já com o milhar do idioma
//! assert_eq!(tn!(1, "{n} linha", "{n} linhas"), "1 linha");
//! assert_eq!(tn!(1234, "{n} linha", "{n} linhas"), "1.234 linhas");
//! assert_eq!(tn!(0, "Uma linha em {t}", "{n} linhas em {t}", t = tabela), "0 linhas em vendas");
//!
//! // em `const` e `static`: o t_estatico! só marca o texto para o extrator (vale o literal); a tradução é na hora
//! // de mostrar, com idioma::traduzir
//! const SALVAR: &str = t_estatico!("Salvar");
//! assert_eq!(idioma::traduzir(SALVAR), "Salvar");
//! # });
//! # idioma::com_idioma(idioma::INGLES, || {
//! // com o inglês valendo (os textos de teste do en.po)
//! assert_eq!(t!("Abrir pasta"), "Open folder");
//! assert_eq!(tn!(1234, "{n} linha", "{n} linhas"), "1,234 rows");
//! assert_eq!(t!("Texto que ninguém traduziu"), "Texto que ninguém traduziu");
//! # });
//! ```
//!
//! - `t!("texto")` e `t!("texto {a}", a = valor, …)` devolvem [`Cow<'static, str>`](std::borrow::Cow): serve direto
//!   no egui (`ui.label(t!("…"))`), no `format!` (`Display`) e em comparações com `&str`; `.into_owned()` quando o
//!   campo pede `String`.
//! - `tn!(n, "singular", "plural", a = valor, …)`: a forma sai da regra do idioma (o `Plural-Forms` do `.po`; o
//!   português usa `n != 1`, então "0 linhas"). O `{n}` é sempre a contagem, com o separador de milhar do idioma, e
//!   não pode ser passado de novo como argumento. O singular pode não ter o `{n}` ("Uma linha").
//! - **Só literais.** A chave precisa ser um texto entre aspas no código: é ele que o extrator (`ferramentas/idioma.rs`)
//!   lê para conferir os catálogos. O compilador confere os nomes: um `{nome}` sem argumento, um argumento que o
//!   texto não usa e um `{}` sem nome são erro de compilação (as macros passam a chave pelo `format_args!` num ramo
//!   que nunca roda, e um `const` confere que cada argumento aparece no texto).
//! - Os `{nome}` não levam formato (`{n:>3}` sai como `{n}`): formate antes e passe o texto pronto
//!   (`t!("Tamanho: {t}", t = formatar_bytes(b))`).
//! - `{{` e `}}` escrevem `{` e `}`, como no `format!`.
//!
//! # `const`, `static` e tabelas
//!
//! O `t!` traduz na hora em que é chamado, então não cabe num `const`. Dois jeitos, nesta ordem de preferência:
//!
//! 1. **Uma função no lugar da constante**, quando o texto é usado em poucos lugares:
//!    `fn dica_salvar() -> Cow<'static, str> { t!("Salvar guarda para as próximas aberturas") }`.
//! 2. **`t_estatico!` na constante e [`traduzir`] na hora de mostrar**, para tabelas (`[(Tema, &str); 3]`) e textos
//!    que já moram em `const`: `const OPCOES: [(char, &str); 2] = [(',', t_estatico!("Vírgula")), …]` e, no desenho,
//!    `traduzir(rotulo)`. O `t_estatico!` não faz nada além de marcar o texto para o extrator. Um `const` que nunca
//!    passa pelo `traduzir` fica em português: é o erro que esse padrão permite, e a revisão da tela em inglês (I4)
//!    pega.
//!
//! O plural de uma tabela: as duas formas com `t_estatico!` e [`escolher_plural`] na hora.
//!
//! # O idioma atual
//!
//! Um valor do processo inteiro ([`atual`], [`definir`]), que a casca troca (as Preferências, o `main` antes do
//! primeiro quadro); os núcleos (`banco`, `esquema`…) traduzem as frases de erro na hora de criá-las, sem conhecer o
//! egui. Lê-lo é barato (um atômico e uma variável da thread). Antes de qualquer [`definir`], vale o português.
//!
//! **Nos testes**, o idioma global é compartilhado pelas threads do `cargo test`: use [`com_idioma`] (ou [`isolar`]),
//! que troca o idioma só da thread que roda o teste, inclusive para o [`definir`] chamado lá dentro (o editor das
//! telas de teste roda na mesma thread).
//!
//! # Um idioma novo
//!
//! Um arquivo `idiomas/<código>.po` (o Poedit, o Weblate e o Crowdin abrem e gravam) e uma linha em [`IDIOMAS`] com
//! o código, o nome no próprio idioma e os formatos de número e data. O resto (o seletor das Preferências, o idioma
//! do sistema, o extrator e os testes dos catálogos) segue a lista. O que faltar traduzir cai no inglês e, sem ele,
//! no português.
//!
//! # O `.po`
//!
//! Lido pelo analisador próprio ([`po`]), no subconjunto que o projeto usa: `msgid`, `msgstr`, `msgid_plural`,
//! `msgstr[n]`, comentários, `#, fuzzy` e o cabeçalho com `Language` e `Plural-Forms`. Um catálogo embutido que não
//! abre é pego pelos testes deste crate; em produção ele só ficaria sem efeito (o português), nunca em pânico.

#![forbid(unsafe_code)]

mod modelo;
pub mod po;
mod sistema;

use std::borrow::Cow;
use std::cell::Cell;
use std::fmt;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

pub use modelo::{Argumento, nomes, preencher};
pub use sistema::{de_locale, do_sistema};

/// Um idioma da interface: o código, o nome e os formatos. Os que existem estão em [`IDIOMAS`].
#[derive(Debug, Clone, Copy)]
pub struct Idioma {
    codigo: &'static str,
    nome: &'static str,
    nome_curto: &'static str,
    milhar: char,
    decimal: char,
    data: &'static str,
    catalogo: Option<&'static str>,
}

impl Idioma {
    /// O código BCP 47 ("pt-BR", "en"): o que as preferências gravam e o nome do `.po`.
    pub fn codigo(&self) -> &'static str {
        self.codigo
    }

    /// O nome no próprio idioma, como o seletor mostra: "Português (Brasil)", "English".
    pub fn nome(&self) -> &'static str {
        self.nome
    }

    /// O nome curto no próprio idioma: "Português", "English" (o "Seguir o sistema (Português)").
    pub fn nome_curto(&self) -> &'static str {
        self.nome_curto
    }

    /// O separador de milhar: '.' no português, ',' no inglês.
    pub fn milhar(&self) -> char {
        self.milhar
    }

    /// O separador decimal: ',' no português, '.' no inglês.
    pub fn decimal(&self) -> char {
        self.decimal
    }

    /// A data no padrão do `strftime` (o do `chrono`): "%d/%m/%Y" no português, "%Y-%m-%d" no inglês. A hora, quando
    /// há, vem depois, com um espaço e "%H:%M" (igual nos dois).
    pub fn padrao_data(&self) -> &'static str {
        self.data
    }

    /// O idioma pelo código, sem diferença de caixa e com `_` ou `-` ("pt-BR", "pt_br", "EN"); None se o DataBox não
    /// o tem.
    pub fn de_codigo(codigo: &str) -> Option<Idioma> {
        let alvo = codigo.trim().replace('_', "-");
        IDIOMAS.iter().copied().find(|i| i.codigo.eq_ignore_ascii_case(&alvo))
    }

    fn indice(&self) -> usize {
        IDIOMAS.iter().position(|i| i.codigo == self.codigo).unwrap_or(0)
    }
}

impl PartialEq for Idioma {
    fn eq(&self, outro: &Idioma) -> bool {
        self.codigo == outro.codigo
    }
}

impl Eq for Idioma {}

impl fmt::Display for Idioma {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.nome)
    }
}

/// O português do Brasil: o idioma das chaves, sem catálogo.
pub const PORTUGUES: Idioma = Idioma {
    codigo: "pt-BR",
    nome: "Português (Brasil)",
    nome_curto: "Português",
    milhar: '.',
    decimal: ',',
    data: "%d/%m/%Y",
    catalogo: None,
};

/// O inglês: o de reserva quando o DataBox não tem o idioma do sistema. As datas no ISO 8601 (2026-10-21), que
/// quem lê inglês em qualquer país entende sem ambiguidade.
pub const INGLES: Idioma = Idioma {
    codigo: "en",
    nome: "English",
    nome_curto: "English",
    milhar: ',',
    decimal: '.',
    data: "%Y-%m-%d",
    catalogo: Some(include_str!("../idiomas/en.po")),
};

/// O registro dos idiomas, na ordem do seletor das Preferências. **Um idioma novo é uma linha aqui** (e o `.po`).
pub const IDIOMAS: &[Idioma] = &[PORTUGUES, INGLES];

// ---------------------------------------------------------------------- o idioma atual

static ATUAL: AtomicUsize = AtomicUsize::new(0);

static CATALOGOS: [OnceLock<po::Catalogo>; IDIOMAS.len()] = [const { OnceLock::new() }; IDIOMAS.len()];

thread_local! {
    /// O idioma desta thread enquanto um [`isolar`] corre (None = o global).
    static ISOLADO: Cell<Option<usize>> = const { Cell::new(None) };
}

fn indice_atual() -> usize {
    ISOLADO.with(Cell::get).unwrap_or_else(|| ATUAL.load(Ordering::Relaxed))
}

/// O idioma que vale agora (o desta thread dentro de um [`isolar`]; senão, o do processo).
pub fn atual() -> Idioma {
    IDIOMAS.get(indice_atual()).copied().unwrap_or(PORTUGUES)
}

/// Troca o idioma do processo (dentro de um [`isolar`], só o da thread). Vale no próximo `t!`: a casca pede um
/// quadro novo para a tela mudar.
pub fn definir(idioma: Idioma) {
    let i = idioma.indice();
    if ISOLADO.with(Cell::get).is_some() {
        ISOLADO.with(|c| c.set(Some(i)));
    } else {
        ATUAL.store(i, Ordering::Relaxed);
    }
}

/// Roda `f` com o idioma desta thread separado do global: começa com o que vale agora, e o [`definir`] de dentro só
/// muda esta thread. Na saída (mesmo com pânico), a thread volta ao que era. Para os testes.
pub fn isolar<R>(f: impl FnOnce() -> R) -> R {
    struct Volta(Option<usize>);
    impl Drop for Volta {
        fn drop(&mut self) {
            ISOLADO.with(|c| c.set(self.0));
        }
    }
    let antes = ISOLADO.with(Cell::get);
    let _volta = Volta(antes);
    ISOLADO.with(|c| c.set(Some(indice_atual())));
    f()
}

/// [`isolar`] já com `idioma`.
pub fn com_idioma<R>(idioma: Idioma, f: impl FnOnce() -> R) -> R {
    isolar(|| {
        definir(idioma);
        f()
    })
}

/// O que a pessoa escolheu nas Preferências: seguir o sistema ou um idioma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Escolha {
    /// O idioma do sistema ([`do_sistema`]), com o inglês quando o DataBox não o tem.
    #[default]
    Sistema,
    Idioma(Idioma),
}

impl Escolha {
    /// "system" | o código do idioma ("pt-BR", "en"): o valor da chave `"idioma"` das preferências.
    pub fn como_str(self) -> &'static str {
        match self {
            Escolha::Sistema => "system",
            Escolha::Idioma(i) => i.codigo,
        }
    }

    /// "system" ou o código de um idioma que o DataBox tem; outro texto → None.
    pub fn de_texto(texto: &str) -> Option<Escolha> {
        if texto == "system" { Some(Escolha::Sistema) } else { Idioma::de_codigo(texto).map(Escolha::Idioma) }
    }

    /// O idioma que a escolha dá agora.
    pub fn resolver(self) -> Idioma {
        match self {
            Escolha::Sistema => do_sistema(),
            Escolha::Idioma(i) => i,
        }
    }

    /// As opções do seletor, na ordem: seguir o sistema e cada idioma de [`IDIOMAS`].
    pub fn todas() -> Vec<Escolha> {
        std::iter::once(Escolha::Sistema).chain(IDIOMAS.iter().copied().map(Escolha::Idioma)).collect()
    }
}

// ---------------------------------------------------------------------- a tradução

fn catalogo(i: usize) -> Option<&'static po::Catalogo> {
    let texto = IDIOMAS.get(i)?.catalogo?;
    let celula = CATALOGOS.get(i)?;
    // um .po embutido que não abre fica sem efeito (os testes do crate garantem que todos abrem)
    Some(celula.get_or_init(|| po::ler(texto).unwrap_or_default()))
}

/// Onde procurar, em ordem: o idioma atual e, se ele não é o inglês nem o português, o inglês (a reserva).
fn cadeia() -> impl Iterator<Item = &'static po::Catalogo> {
    let i = indice_atual();
    let reserva = INGLES.indice();
    let segundo = (i != reserva && i != PORTUGUES.indice()).then_some(reserva);
    std::iter::once(i).chain(segundo).filter_map(catalogo)
}

/// A tradução de um texto marcado com `t_estatico!` (ou de qualquer chave fixa) no idioma atual; sem tradução, o
/// próprio texto.
pub fn traduzir(chave: &'static str) -> &'static str {
    cadeia().find_map(|c| c.simples(chave)).unwrap_or(chave)
}

/// A forma do plural para `n` no idioma atual: a do catálogo ou, sem tradução, `um`/`varios` pela regra do
/// português (`n != 1`). Não troca nenhum `{n}`: é o que o `tn!` usa por baixo, e o que serve às tabelas com as duas
/// formas em `t_estatico!`.
pub fn escolher_plural<'a>(n: u64, um: &'a str, varios: &'a str) -> &'a str {
    if let Some(forma) = cadeia().find_map(|c| c.plural(um, n)) {
        return forma;
    }
    if n == 1 { um } else { varios }
}

/// `1234` → "1.234" (o milhar do idioma atual): o `{n}` do `tn!`.
pub fn contagem(n: u64) -> String {
    let digitos = n.to_string();
    let milhar = atual().milhar;
    let mut s = String::with_capacity(digitos.len() + digitos.len() / 3);
    for (i, c) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i).is_multiple_of(3) {
            s.push(milhar);
        }
        s.push(c);
    }
    s
}

/// O texto tem `{nome}` (ou `{nome:…}`): a conferência, na compilação, de que todo argumento do `t!`/`tn!` é usado.
#[doc(hidden)]
pub const fn __usa(texto: &str, nome: &str) -> bool {
    let (t, n) = (texto.as_bytes(), nome.as_bytes());
    let mut i = 0;
    while i + n.len() + 2 <= t.len() {
        if t[i] == b'{' {
            let mut k = 0;
            while k < n.len() && t[i + 1 + k] == n[k] {
                k += 1;
            }
            if k == n.len() && (t[i + 1 + k] == b'}' || t[i + 1 + k] == b':') {
                return true;
            }
        }
        i += 1;
    }
    false
}

#[doc(hidden)]
pub fn __t(chave: &'static str) -> Cow<'static, str> {
    Cow::Borrowed(traduzir(chave))
}

#[doc(hidden)]
pub fn __t_com(chave: &'static str, args: &[Argumento<'_>]) -> Cow<'static, str> {
    Cow::Owned(preencher(traduzir(chave), args))
}

#[doc(hidden)]
pub fn __tn(n: u64, um: &'static str, varios: &'static str, args: &[Argumento<'_>]) -> Cow<'static, str> {
    let forma = escolher_plural(n, um, varios);
    if !forma.contains(['{', '}']) {
        return Cow::Borrowed(forma);
    }
    let numero = contagem(n);
    let mut todos: Vec<Argumento<'_>> = Vec::with_capacity(args.len() + 1);
    todos.push(("n", &numero));
    todos.extend_from_slice(args);
    Cow::Owned(preencher(forma, &todos))
}

/// A contagem do `tn!`: qualquer inteiro (os negativos contam pelo valor absoluto; acima de `u64`, satura).
pub trait Contagem {
    fn contagem(self) -> u64;
}

macro_rules! contagem_sem_sinal {
    ($($t:ty),*) => {$(
        impl Contagem for $t {
            fn contagem(self) -> u64 {
                u64::try_from(self).unwrap_or(u64::MAX)
            }
        }
    )*};
}
macro_rules! contagem_com_sinal {
    ($($t:ty),*) => {$(
        impl Contagem for $t {
            fn contagem(self) -> u64 {
                u64::try_from(self.unsigned_abs()).unwrap_or(u64::MAX)
            }
        }
    )*};
}
contagem_sem_sinal!(u8, u16, u32, u64, u128, usize);
contagem_com_sinal!(i8, i16, i32, i64, i128, isize);

impl<T: Contagem + Copy> Contagem for &T {
    fn contagem(self) -> u64 {
        (*self).contagem()
    }
}

/// O texto no idioma atual. Ver o topo do crate.
///
/// ```
/// # use idioma::t;
/// let n = 3;
/// assert_eq!(t!("Abrir pasta"), "Abrir pasta");
/// assert_eq!(t!("{n} de {total}", n = n, total = 10), "3 de 10");
/// ```
///
/// O compilador recusa um `{nome}` sem argumento e um argumento sobrando:
///
/// ```compile_fail
/// # use idioma::t;
/// let _ = t!("Gravado em {tabela}");
/// ```
///
/// ```compile_fail
/// # use idioma::t;
/// let _ = t!("Gravado", tabela = "x");
/// ```
#[macro_export]
macro_rules! t {
    ($texto:literal $(,)?) => {{
        if false {
            let _ = ::std::format_args!(::std::concat!($texto));
        }
        $crate::__t($texto)
    }};
    ($texto:literal $(, $nome:ident = $valor:expr)+ $(,)?) => {{
        if false {
            let _ = ::std::format_args!(
                ::std::concat!($texto $(, "{", ::std::stringify!($nome), "}")+),
                $($nome = $valor),+
            );
        }
        $(const _: () = ::std::assert!(
            $crate::__usa($texto, ::std::stringify!($nome)),
            ::std::concat!("o texto do t! não usa o argumento {{", ::std::stringify!($nome), "}}")
        );)+
        $crate::__t_com($texto, &[$((::std::stringify!($nome), &$valor as &dyn ::std::fmt::Display)),+])
    }};
}

/// O plural no idioma atual: `tn!(n, "{n} linha", "{n} linhas")`. Ver o topo do crate.
///
/// ```
/// # use idioma::tn;
/// assert_eq!(tn!(2usize, "{n} arquivo", "{n} arquivos"), "2 arquivos");
/// assert_eq!(tn!(1, "Um arquivo em {pasta}", "{n} arquivos em {pasta}", pasta = "dados"), "Um arquivo em dados");
/// ```
///
/// ```compile_fail
/// # use idioma::tn;
/// // o {n} é da contagem: não se passa de novo
/// let _ = tn!(2, "{n} linha", "{n} linhas", n = 5);
/// ```
#[macro_export]
macro_rules! tn {
    ($n:expr, $um:literal, $varios:literal $(, $nome:ident = $valor:expr)* $(,)?) => {{
        if false {
            let _ = ::std::format_args!(
                ::std::concat!($um, "{n}" $(, "{", ::std::stringify!($nome), "}")*),
                n = 0u64 $(, $nome = $valor)*
            );
            let _ = ::std::format_args!(
                ::std::concat!($varios, "{n}" $(, "{", ::std::stringify!($nome), "}")*),
                n = 0u64 $(, $nome = $valor)*
            );
        }
        $(const _: () = ::std::assert!(
            $crate::__usa($um, ::std::stringify!($nome)) || $crate::__usa($varios, ::std::stringify!($nome)),
            ::std::concat!("nenhuma das formas do tn! usa o argumento {{", ::std::stringify!($nome), "}}")
        );)*
        $crate::__tn(
            $crate::Contagem::contagem($n),
            $um,
            $varios,
            &[$((::std::stringify!($nome), &$valor as &dyn ::std::fmt::Display)),*],
        )
    }};
}

/// Marca um texto para o extrator e devolve o próprio literal, num `const` ou `static`; a tradução é na hora de
/// mostrar, com [`traduzir`] (ou [`escolher_plural`]). Ver o topo do crate.
///
/// ```
/// # use idioma::t_estatico;
/// const SEPARADORES: [(char, &str); 2] = [(';', t_estatico!("Ponto e vírgula")), (',', t_estatico!("Vírgula"))];
/// assert_eq!(idioma::traduzir(SEPARADORES[1].1), "Vírgula");
/// ```
#[macro_export]
macro_rules! t_estatico {
    ($texto:literal $(,)?) => {{
        const TEXTO: &str = $texto;
        TEXTO
    }};
}

#[cfg(test)]
mod testes;
