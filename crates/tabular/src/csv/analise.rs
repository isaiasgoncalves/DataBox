//! O analisador de registros, sobre bytes: a única definição do que é um registro e um campo, usada pelo índice,
//! pela leitura das páginas, pela gravação em fluxo e pela leitura do TSV da área de transferência.
//!
//! As regras são as do CSV comum, sem modo estrito (o que o Excel e o `exportar` produzem lê igual):
//! - o registro acaba em `\r\n`, `\n` ou `\r` fora de aspas; o fim do arquivo também o encerra, e uma quebra no
//!   fim do arquivo não cria um registro a mais;
//! - o campo que começa com `"` é citado: vai até a próxima `"` que não seja dobrada (`""` é uma aspa); dentro
//!   dele o separador e as quebras de linha são texto; depois da aspa de fechamento, o que vier até o próximo
//!   separador é texto solto, sem desfazer aspas;
//! - aspa no meio de um campo que não começou com aspa é uma aspa comum;
//! - linha em branco é um registro de um campo vazio; campos a mais ou a menos que os do vizinho são aceitos
//!   (registro irregular);
//! - aspa sem fechar vai até o fim do arquivo.

use std::borrow::Cow;
use std::io::{self, Read};

use super::codificacao::{Codificacao, ValidadorUtf8};

/// Onde um campo está no buffer (com as aspas, se tiver).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Campo {
    pub ini: usize,
    pub fim: usize,
    pub citado: bool,
}

/// Um registro achado no buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Registro {
    /// Fim do conteúdo, sem a quebra de linha.
    pub fim_conteudo: usize,
    /// Fim do registro, depois da quebra de linha (o começo do próximo).
    pub fim: usize,
    pub n_campos: u32,
}

/// Acha o registro que começa em `ini`. `eof` diz que o buffer vai até o fim do arquivo; sem ele, um registro
/// que pode continuar além do buffer (inclusive um `\r` no último byte, que pode ser o começo de um `\r\n`)
/// devolve `None`, e quem chama traz mais bytes e tenta de novo. Com `campos`, anota onde cada campo está.
/// Quem chama garante que `ini < buf.len()` ou que ainda faltam bytes.
pub(crate) fn proximo_registro(
    buf: &[u8],
    ini: usize,
    sep: u8,
    eof: bool,
    mut campos: Option<&mut Vec<Campo>>,
) -> Option<Registro> {
    if let Some(c) = campos.as_deref_mut() {
        c.clear();
    }
    let n = buf.len();
    let mut pos = ini;
    let mut n_campos: u32 = 0;
    loop {
        let campo_ini = pos;
        let mut citado = false;
        if buf.get(pos) == Some(&b'"') {
            citado = true;
            pos += 1;
            loop {
                let Some(k) = buf[pos..].iter().position(|&b| b == b'"') else {
                    if !eof {
                        return None;
                    }
                    pos = n; // aspa sem fechar: vai até o fim do arquivo
                    break;
                };
                pos += k + 1;
                match buf.get(pos) {
                    Some(b'"') => pos += 1, // aspa dobrada
                    Some(_) => break,
                    None => {
                        if !eof {
                            return None;
                        }
                        break;
                    }
                }
            }
        }
        // o resto do campo, até o separador ou o fim do registro
        let Some(k) = buf[pos..].iter().position(|&b| b == sep || b == b'\n' || b == b'\r') else {
            if !eof {
                return None;
            }
            if let Some(c) = campos.as_deref_mut() {
                c.push(Campo { ini: campo_ini, fim: n, citado });
            }
            return Some(Registro { fim_conteudo: n, fim: n, n_campos: n_campos + 1 });
        };
        let p = pos + k;
        if let Some(c) = campos.as_deref_mut() {
            c.push(Campo { ini: campo_ini, fim: p, citado });
        }
        n_campos += 1;
        let b = buf[p];
        if b == sep {
            pos = p + 1;
            continue;
        }
        let fim = if b == b'\n' {
            p + 1
        } else {
            match buf.get(p + 1) {
                Some(b'\n') => p + 2,
                Some(_) => p + 1,
                None => {
                    if !eof {
                        return None;
                    }
                    p + 1
                }
            }
        };
        return Some(Registro { fim_conteudo: p, fim, n_campos });
    }
}

/// O texto de um campo: desfaz as aspas (`""` vira `"`), tira as de fora e decodifica.
pub(crate) fn decodificar_campo(buf: &[u8], campo: Campo, cod: Codificacao) -> String {
    let Some(bytes) = buf.get(campo.ini..campo.fim) else {
        return String::new();
    };
    if !campo.citado {
        return cod.decodificar(bytes).into_owned();
    }
    cod.decodificar(&desfazer_aspas(bytes)).into_owned()
}

fn desfazer_aspas(bytes: &[u8]) -> Cow<'_, [u8]> {
    let mut saida = Vec::with_capacity(bytes.len());
    let mut i = 1; // pula a aspa de abertura
    loop {
        let Some(k) = bytes.get(i..).and_then(|r| r.iter().position(|&b| b == b'"')) else {
            saida.extend_from_slice(bytes.get(i..).unwrap_or_default());
            break;
        };
        saida.extend_from_slice(&bytes[i..i + k]);
        i += k + 1;
        if bytes.get(i) == Some(&b'"') {
            saida.push(b'"');
            i += 1;
        } else {
            // aspa de fechamento: o que sobrar é texto solto
            saida.extend_from_slice(bytes.get(i..).unwrap_or_default());
            break;
        }
    }
    Cow::Owned(saida)
}

/// Tamanho do primeiro bloco lido do arquivo.
pub(crate) const BLOCO: usize = 1 << 20;

/// Percorre os registros de um fluxo (um arquivo, ou um trecho dele) um a um, com um buffer que se arrasta.
/// Quem lê vê o registro atual por referência: nada é copiado enquanto não for preciso.
pub(crate) struct Varredura<R: Read> {
    fonte: R,
    buf: Vec<u8>,
    cheio: usize,
    /// Onde começa o próximo registro no buffer.
    pos: usize,
    /// A posição no arquivo do byte 0 do buffer.
    deslocamento: u64,
    eof: bool,
    sep: u8,
    coletar: bool,
    campos: Vec<Campo>,
    atual: Registro,
    ini_atual: usize,
    validador: Option<ValidadorUtf8>,
}

impl<R: Read> Varredura<R> {
    /// `deslocamento` é a posição do arquivo onde a `fonte` começa (para as posições dos registros);
    /// `coletar` liga a anotação dos campos; `validar_utf8` confere o texto todo enquanto o lê.
    pub fn nova(
        fonte: R,
        deslocamento: u64,
        sep: u8,
        coletar: bool,
        validar_utf8: bool,
        capacidade: usize,
    ) -> Varredura<R> {
        Varredura {
            fonte,
            buf: vec![0; capacidade.max(64)],
            cheio: 0,
            pos: 0,
            deslocamento,
            eof: false,
            sep,
            coletar,
            campos: Vec::new(),
            atual: Registro { fim_conteudo: 0, fim: 0, n_campos: 0 },
            ini_atual: 0,
            validador: validar_utf8.then(ValidadorUtf8::default),
        }
    }

    fn ler_mais(&mut self) -> io::Result<()> {
        if self.pos > 0 {
            self.buf.copy_within(self.pos..self.cheio, 0);
            self.cheio -= self.pos;
            self.deslocamento += self.pos as u64;
            self.pos = 0;
        }
        if self.cheio == self.buf.len() {
            let novo = self.buf.len() * 2;
            self.buf.resize(novo, 0);
        }
        loop {
            match self.fonte.read(&mut self.buf[self.cheio..]) {
                Ok(0) => {
                    self.eof = true;
                    return Ok(());
                }
                Ok(k) => {
                    if let Some(v) = self.validador.as_mut() {
                        v.alimentar(&self.buf[self.cheio..self.cheio + k]);
                    }
                    self.cheio += k;
                    return Ok(());
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }

    /// Avança para o próximo registro; `false` quando acabaram.
    pub fn seguinte(&mut self) -> io::Result<bool> {
        self.pos = self.atual.fim.max(self.pos);
        loop {
            if self.eof && self.pos >= self.cheio {
                return Ok(false);
            }
            if self.pos < self.cheio {
                let campos = if self.coletar { Some(&mut self.campos) } else { None };
                if let Some(r) = proximo_registro(&self.buf[..self.cheio], self.pos, self.sep, self.eof, campos) {
                    self.ini_atual = self.pos;
                    self.atual = r;
                    return Ok(true);
                }
            }
            if self.eof {
                return Ok(false);
            }
            self.ler_mais()?;
        }
    }

    /// Pula `n` registros; devolve quantos pulou (menos que `n` se o fluxo acabou).
    pub fn pular(&mut self, n: u64) -> io::Result<u64> {
        let mut pulados = 0;
        while pulados < n && self.seguinte()? {
            pulados += 1;
        }
        Ok(pulados)
    }

    /// A posição, no arquivo, onde o registro atual começa.
    pub fn deslocamento_do_registro(&self) -> u64 {
        self.deslocamento + self.ini_atual as u64
    }

    /// A posição, no arquivo, logo depois do registro atual.
    pub fn deslocamento_do_fim(&self) -> u64 {
        self.deslocamento + self.atual.fim as u64
    }

    pub fn registro(&self) -> Registro {
        self.atual
    }

    /// O registro atual, sem a quebra de linha.
    pub fn conteudo(&self) -> &[u8] {
        self.buf.get(self.ini_atual..self.atual.fim_conteudo).unwrap_or_default()
    }

    /// A quebra de linha que fecha o registro atual (vazia no último, se o arquivo não termina em quebra).
    pub fn terminador(&self) -> &[u8] {
        self.buf.get(self.atual.fim_conteudo..self.atual.fim).unwrap_or_default()
    }

    pub fn campos(&self) -> &[Campo] {
        &self.campos
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buf[..self.cheio]
    }

    /// Todos os campos do registro atual, decodificados. Exige `coletar`.
    pub fn todos_os_campos(&self, cod: Codificacao) -> Vec<String> {
        let buf = &self.buf[..self.cheio];
        self.campos.iter().map(|c| decodificar_campo(buf, *c, cod)).collect()
    }

    /// Se o texto lido até aqui é UTF-8 válido do começo ao fim (só com `validar_utf8`; vale depois que o fluxo
    /// acabou).
    pub fn utf8_valido(&self) -> bool {
        self.validador.as_ref().is_none_or(ValidadorUtf8::valido)
    }
}

/// Os registros de um trecho de bytes (em memória), cada um como a lista dos textos dos campos. `truncado`
/// diz que o trecho é só o começo de um arquivo maior: o último registro, que pode estar cortado, é
/// descartado. No máximo `maximo` registros.
pub(crate) fn registros_do_trecho(
    bytes: &[u8],
    sep: u8,
    truncado: bool,
    maximo: usize,
    cod: Codificacao,
) -> Vec<Vec<String>> {
    let mut saida = Vec::new();
    let mut campos = Vec::new();
    let mut pos = 0;
    while saida.len() < maximo && pos < bytes.len() {
        let Some(r) = proximo_registro(bytes, pos, sep, !truncado, Some(&mut campos)) else {
            break;
        };
        saida.push(campos.iter().map(|c| decodificar_campo(bytes, *c, cod)).collect());
        pos = r.fim;
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    fn registros(texto: &[u8], sep: u8) -> Vec<Vec<String>> {
        registros_do_trecho(texto, sep, false, usize::MAX, Codificacao::Utf8)
    }

    fn r(linhas: &[&[&str]]) -> Vec<Vec<String>> {
        linhas.iter().map(|l| l.iter().map(|c| c.to_string()).collect()).collect()
    }

    #[test]
    fn registros_simples_com_cada_quebra() {
        assert_eq!(registros(b"a;b\r\nc;d\r\n", b';'), r(&[&["a", "b"], &["c", "d"]]));
        assert_eq!(registros(b"a;b\nc;d\n", b';'), r(&[&["a", "b"], &["c", "d"]]));
        assert_eq!(registros(b"a;b\rc;d\r", b';'), r(&[&["a", "b"], &["c", "d"]]));
        assert_eq!(registros(b"a;b\r\nc;d", b';'), r(&[&["a", "b"], &["c", "d"]]), "sem quebra no fim");
        assert_eq!(registros(b"a;b\nc;d\r\ne;f\r", b';'), r(&[&["a", "b"], &["c", "d"], &["e", "f"]]), "misturadas");
    }

    #[test]
    fn arquivo_vazio_e_so_quebra() {
        assert_eq!(registros(b"", b';'), Vec::<Vec<String>>::new());
        assert_eq!(registros(b"\r\n", b';'), r(&[&[""]]));
        assert_eq!(registros(b"\n\n", b';'), r(&[&[""], &[""]]));
        assert_eq!(registros(b"a\n\nb\n", b';'), r(&[&["a"], &[""], &["b"]]), "linha em branco é um registro");
    }

    #[test]
    fn campos_vazios_e_separador_no_fim() {
        assert_eq!(registros(b"a;;c\n", b';'), r(&[&["a", "", "c"]]));
        assert_eq!(registros(b"a;b;\n", b';'), r(&[&["a", "b", ""]]));
        assert_eq!(registros(b"a;b;", b';'), r(&[&["a", "b", ""]]), "separador no último byte do arquivo");
        assert_eq!(registros(b";\n", b';'), r(&[&["", ""]]));
    }

    #[test]
    fn aspas_separador_e_quebra_dentro() {
        assert_eq!(registros(b"\"a;b\";c\n", b';'), r(&[&["a;b", "c"]]));
        assert_eq!(
            registros(b"\"linha1\r\nlinha2\";c\r\nd;e\r\n", b';'),
            r(&[&["linha1\r\nlinha2", "c"], &["d", "e"]])
        );
        assert_eq!(registros(b"\"a\nb\"\n", b';'), r(&[&["a\nb"]]));
        assert_eq!(registros(b"\"ele disse \"\"oi\"\"\";x\n", b';'), r(&[&["ele disse \"oi\"", "x"]]));
        assert_eq!(registros(b"\"\"\"\";x\n", b';'), r(&[&["\"", "x"]]), "só uma aspa dobrada");
        assert_eq!(registros(b"\"\";x\n", b';'), r(&[&["", "x"]]), "campo citado vazio");
        assert_eq!(registros(b"\"\"\n", b';'), r(&[&[""]]));
    }

    #[test]
    fn aspas_fora_do_lugar_sao_lenientes() {
        assert_eq!(registros(b"5\" cano;x\n", b';'), r(&[&["5\" cano", "x"]]), "aspa no meio é aspa comum");
        assert_eq!(registros(b"\"ab\"cd;x\n", b';'), r(&[&["abcd", "x"]]), "texto solto depois do fechamento");
        assert_eq!(registros(b"\"ab\"c\"d;x\n", b';'), r(&[&["abc\"d", "x"]]));
        assert_eq!(registros(b" \"ab\";x\n", b';'), r(&[&[" \"ab\"", "x"]]), "espaço antes da aspa: campo comum");
        assert_eq!(registros(b"\"sem fechar;x\ny;z\n", b';'), r(&[&["sem fechar;x\ny;z\n"]]), "aspa sem fechar");
        assert_eq!(registros(b"\"a\"", b';'), r(&[&["a"]]), "aspa de fechamento no último byte");
    }

    #[test]
    fn outros_separadores() {
        assert_eq!(registros(b"a,b\n\"x,y\",z\n", b','), r(&[&["a", "b"], &["x,y", "z"]]));
        assert_eq!(registros(b"a\tb\n", b'\t'), r(&[&["a", "b"]]));
        assert_eq!(registros(b"a|b\n", b'|'), r(&[&["a", "b"]]));
        assert_eq!(registros(b"a;b\n", b','), r(&[&["a;b"]]));
    }

    #[test]
    fn campos_irregulares() {
        assert_eq!(registros(b"a;b;c\nd\ne;f;g;h\n", b';'), r(&[&["a", "b", "c"], &["d"], &["e", "f", "g", "h"]]));
    }

    #[test]
    fn acentos_nas_duas_codificacoes() {
        let utf8 = registros_do_trecho("ação;Ñ\n".as_bytes(), b';', false, 10, Codificacao::Utf8);
        assert_eq!(utf8, r(&[&["ação", "Ñ"]]));
        let cp = registros_do_trecho(b"a\xE7\xE3o;\xD1\n\"\xE3\"\n", b';', false, 10, Codificacao::Windows1252);
        assert_eq!(cp, r(&[&["ação", "Ñ"], &["ã"]]));
    }

    #[test]
    fn trecho_truncado_descarta_o_registro_cortado() {
        let t = b"a;b\nc;d\ne;";
        assert_eq!(registros_do_trecho(t, b';', true, 10, Codificacao::Utf8), r(&[&["a", "b"], &["c", "d"]]));
        let t = b"a;b\nc;d\r";
        assert_eq!(
            registros_do_trecho(t, b';', true, 10, Codificacao::Utf8),
            r(&[&["a", "b"]]),
            "o \\r final pode ser o começo de um \\r\\n"
        );
        assert_eq!(registros_do_trecho(t, b';', false, 10, Codificacao::Utf8), r(&[&["a", "b"], &["c", "d"]]));
        let t = b"a;\"b\nc";
        assert_eq!(registros_do_trecho(t, b';', true, 10, Codificacao::Utf8), Vec::<Vec<String>>::new());
        assert_eq!(
            registros_do_trecho(b"a\nb\nc\n", b';', false, 2, Codificacao::Utf8),
            r(&[&["a"], &["b"]]),
            "máximo"
        );
    }

    #[test]
    fn posicoes_do_registro() {
        let texto = b"ab;c\r\n\"x\ny\";z\nfim";
        let mut campos = Vec::new();
        let r1 = proximo_registro(texto, 0, b';', true, Some(&mut campos)).unwrap();
        assert_eq!((r1.fim_conteudo, r1.fim, r1.n_campos), (4, 6, 2));
        assert_eq!(campos, [Campo { ini: 0, fim: 2, citado: false }, Campo { ini: 3, fim: 4, citado: false }]);
        let r2 = proximo_registro(texto, r1.fim, b';', true, Some(&mut campos)).unwrap();
        assert_eq!((r2.fim_conteudo, r2.fim, r2.n_campos), (13, 14, 2));
        assert!(campos[0].citado);
        let r3 = proximo_registro(texto, r2.fim, b';', true, None).unwrap();
        assert_eq!((r3.fim_conteudo, r3.fim, r3.n_campos), (17, 17, 1));
        assert_eq!(r1.fim - r1.fim_conteudo, 2);
        assert_eq!(r2.fim - r2.fim_conteudo, 1);
        assert_eq!(r3.fim - r3.fim_conteudo, 0);
    }

    /// Fonte que entrega no máximo `passo` bytes por leitura, para exercitar os cortes entre blocos.
    struct Pingando<'a> {
        dados: &'a [u8],
        passo: usize,
    }

    impl Read for Pingando<'_> {
        fn read(&mut self, saida: &mut [u8]) -> io::Result<usize> {
            let n = self.dados.len().min(self.passo).min(saida.len());
            saida[..n].copy_from_slice(&self.dados[..n]);
            self.dados = &self.dados[n..];
            Ok(n)
        }
    }

    fn varrer(texto: &[u8], passo: usize, capacidade: usize) -> (Vec<Vec<String>>, Vec<u64>) {
        let mut v = Varredura::nova(Pingando { dados: texto, passo }, 10, b';', true, true, capacidade);
        let mut registros = Vec::new();
        let mut posicoes = Vec::new();
        while v.seguinte().unwrap() {
            posicoes.push(v.deslocamento_do_registro());
            registros.push(v.todos_os_campos(Codificacao::Utf8));
        }
        assert!(v.utf8_valido());
        (registros, posicoes)
    }

    #[test]
    fn varredura_igual_ao_trecho_em_qualquer_corte() {
        let texto =
            "id;nome;obs\r\n1;\"Ana\r\nSilva\";\"ele disse \"\"oi\"\"\"\r\n2;José;\n\n3;\"ç\";x\r4;fim;y".as_bytes();
        let esperado = registros(texto, b';');
        let (primeira, posicoes) = varrer(texto, usize::MAX, 1 << 20);
        assert_eq!(primeira, esperado);
        assert_eq!(posicoes[0], 10);
        assert_eq!(posicoes.len(), esperado.len());
        for passo in 1..=9 {
            for capacidade in [64, 100, 1 << 10] {
                let (v, p) = varrer(texto, passo, capacidade);
                assert_eq!(v, esperado, "passo {passo}, capacidade {capacidade}");
                assert_eq!(p, posicoes, "posições, passo {passo}, capacidade {capacidade}");
            }
        }
    }

    #[test]
    fn varredura_com_registro_maior_que_o_buffer() {
        let mut texto = b"a;\"".to_vec();
        texto.extend(std::iter::repeat_n(b'x', 5000));
        texto.extend_from_slice(b"\";b\nc;d\n");
        let (v, _) = varrer(&texto, 100, 64);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0][1].len(), 5000);
        assert_eq!(v[1], ["c", "d"]);
    }

    #[test]
    fn varredura_pula_e_diz_onde_acaba() {
        let texto = b"a\nbb\nccc\ndddd\n";
        let mut v = Varredura::nova(&texto[..], 0, b';', false, false, 64);
        assert_eq!(v.pular(2).unwrap(), 2);
        assert!(v.seguinte().unwrap());
        assert_eq!(v.conteudo(), b"ccc");
        assert_eq!(v.terminador(), b"\n");
        assert_eq!(v.deslocamento_do_registro(), 5);
        assert_eq!(v.deslocamento_do_fim(), 9);
        assert_eq!(v.pular(10).unwrap(), 1);
        assert!(!v.seguinte().unwrap());
    }

    #[test]
    fn varredura_sem_quebra_no_fim_e_utf8_invalido() {
        let mut v = Varredura::nova(&b"a;b\nc;\xE7"[..], 0, b';', true, true, 64);
        assert!(v.seguinte().unwrap() && v.seguinte().unwrap());
        assert_eq!(v.terminador(), b"");
        assert_eq!(v.todos_os_campos(Codificacao::Windows1252), ["c", "ç"]);
        assert!(!v.seguinte().unwrap());
        assert!(!v.utf8_valido());
    }
}
