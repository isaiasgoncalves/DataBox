//! O índice dos registros de um arquivo: uma passada só, guardando a posição de um registro a cada
//! [`PASSO_INDICE`] (uns 128 KB por milhão de linhas). Ler uma página é ir à posição do ponto mais próximo e
//! percorrer, no máximo, 63 registros.

use std::io::Read;
use std::path::Path;

use comum::Resultado;

use super::analise::{BLOCO, Varredura};
use super::arquivos::{CONTEXTO_LER, erro_io};

/// De quantos em quantos registros o índice guarda a posição.
pub const PASSO_INDICE: u64 = 64;

/// De quantos em quantos bytes a passada avisa o progresso.
const PASSO_DO_AVISO: u64 = 4 << 20;

/// O que uma passada pelo arquivo todo descobre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Indice {
    /// Quantos registros (o cabeçalho, se houver, é o primeiro).
    pub registros: u64,
    /// A posição, no arquivo, do registro `k * PASSO_INDICE`.
    pontos: Vec<u64>,
    /// O tamanho do arquivo, em bytes.
    pub tamanho: u64,
    /// Onde o primeiro registro começa (3 depois do BOM).
    pub inicio: u64,
    /// O maior número de campos de um registro.
    pub max_campos: u32,
    /// Quantos registros têm um número de campos diferente do primeiro.
    pub irregulares: u64,
    /// O último registro termina com a quebra de linha (verdade num arquivo sem registros).
    pub termina_em_quebra: bool,
}

impl Indice {
    pub fn pontos(&self) -> &[u64] {
        &self.pontos
    }

    /// O ponto de onde começar a percorrer para chegar ao `registro`: a posição do ponto e quantos registros
    /// pular a partir dela. `None` se o registro não existe.
    pub(crate) fn onde_comeca(&self, registro: u64) -> Option<(u64, u64)> {
        if registro >= self.registros {
            return None;
        }
        let k = usize::try_from(registro / PASSO_INDICE).ok()?;
        Some((*self.pontos.get(k)?, registro % PASSO_INDICE))
    }

    /// A posição onde terminam os registros até o `ultimo` (a do próximo ponto, ou o fim do arquivo).
    pub(crate) fn onde_acaba(&self, ultimo: u64) -> u64 {
        let k = usize::try_from(ultimo / PASSO_INDICE + 1).unwrap_or(usize::MAX);
        self.pontos.get(k).copied().unwrap_or(self.tamanho)
    }
}

/// Monta o [`Indice`] registro a registro: na passada de abertura e na gravação, que conhece as posições do
/// arquivo novo e dispensa uma segunda passada.
#[derive(Debug, Default)]
pub(crate) struct ConstrutorIndice {
    registros: u64,
    pontos: Vec<u64>,
    primeiro: Option<u32>,
    max: u32,
    irregulares: u64,
}

impl ConstrutorIndice {
    pub fn registro(&mut self, deslocamento: u64, n_campos: u32) {
        if self.registros.is_multiple_of(PASSO_INDICE) {
            self.pontos.push(deslocamento);
        }
        self.registros += 1;
        self.max = self.max.max(n_campos);
        match self.primeiro {
            None => self.primeiro = Some(n_campos),
            Some(p) if p != n_campos => self.irregulares += 1,
            Some(_) => {}
        }
    }

    pub fn concluir(self, inicio: u64, tamanho: u64, termina_em_quebra: bool) -> Indice {
        Indice {
            registros: self.registros,
            pontos: self.pontos,
            tamanho,
            inicio,
            max_campos: self.max,
            irregulares: self.irregulares,
            termina_em_quebra: termina_em_quebra || self.registros == 0,
        }
    }
}

/// O resultado da passada de abertura.
pub(crate) struct Varrido {
    pub indice: Indice,
    /// O texto do arquivo todo é UTF-8 válido.
    pub utf8_valido: bool,
}

/// Percorre `fonte` (posicionada em `inicio`) até o fim. `ao_progresso(bytes, total)` é chamado de tempos em
/// tempos e uma última vez no fim; se devolver erro, a passada para e o erro sobe como veio.
pub(crate) fn construir_indice<R: Read>(
    fonte: R,
    caminho: &Path,
    inicio: u64,
    tamanho: u64,
    sep: u8,
    ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
) -> Resultado<Varrido> {
    let mut v = Varredura::nova(fonte, inicio, sep, false, true, BLOCO);
    let mut construtor = ConstrutorIndice::default();
    let mut termina = true;
    let mut proximo_aviso = inicio + PASSO_DO_AVISO;
    loop {
        match v.seguinte() {
            Ok(true) => {}
            Ok(false) => break,
            Err(e) => return Err(erro_io(&e, Some(caminho), CONTEXTO_LER)),
        }
        let r = v.registro();
        construtor.registro(v.deslocamento_do_registro(), r.n_campos);
        termina = r.fim > r.fim_conteudo;
        if v.deslocamento_do_fim() >= proximo_aviso {
            ao_progresso(v.deslocamento_do_fim(), tamanho)?;
            proximo_aviso = v.deslocamento_do_fim() + PASSO_DO_AVISO;
        }
    }
    ao_progresso(tamanho, tamanho)?;
    Ok(Varrido { indice: construtor.concluir(inicio, tamanho, termina), utf8_valido: v.utf8_valido() })
}

#[cfg(test)]
mod testes {
    use std::io::Cursor;

    use super::*;

    fn indice(texto: &[u8], sep: u8) -> Varrido {
        let mut sem_aviso = |_: u64, _: u64| Ok(());
        construir_indice(Cursor::new(texto), Path::new("x.csv"), 0, texto.len() as u64, sep, &mut sem_aviso).unwrap()
    }

    #[test]
    fn conta_registros_campos_e_irregulares() {
        let v = indice(b"a;b;c\r\n1;2;3\r\n4;5\r\n6;7;8;9\r\n\r\n", b';');
        let i = &v.indice;
        assert_eq!(i.registros, 5);
        assert_eq!(i.max_campos, 4);
        assert_eq!(i.irregulares, 3, "4;5 / 6;7;8;9 / linha em branco");
        assert!(i.termina_em_quebra && v.utf8_valido);
        assert_eq!(i.pontos(), &[0]);
        assert_eq!(i.tamanho, 30);
    }

    #[test]
    fn arquivo_sem_quebra_no_fim_e_arquivo_vazio() {
        assert!(!indice(b"a;b\r\nc;d", b';').indice.termina_em_quebra);
        assert!(indice(b"a;b\r\nc;d\n", b';').indice.termina_em_quebra);
        let vazio = indice(b"", b';');
        assert_eq!(vazio.indice.registros, 0);
        assert!(vazio.indice.termina_em_quebra && vazio.indice.pontos().is_empty());
        assert_eq!(indice(b"\n", b';').indice.registros, 1);
    }

    #[test]
    fn pontos_a_cada_passo() {
        let mut texto = Vec::new();
        let mut inicios = Vec::new();
        for i in 0..200 {
            inicios.push(texto.len() as u64);
            texto.extend_from_slice(format!("{i};linha {i}\n").as_bytes());
        }
        let i = indice(&texto, b';').indice;
        assert_eq!(i.registros, 200);
        assert_eq!(i.pontos(), &[inicios[0], inicios[64], inicios[128], inicios[192]]);
        assert_eq!(i.onde_comeca(0), Some((inicios[0], 0)));
        assert_eq!(i.onde_comeca(130), Some((inicios[128], 2)));
        assert_eq!(i.onde_comeca(199), Some((inicios[192], 7)));
        assert_eq!(i.onde_comeca(200), None);
        assert_eq!(i.onde_acaba(5), inicios[64]);
        assert_eq!(i.onde_acaba(63), inicios[64]);
        assert_eq!(i.onde_acaba(64), inicios[128]);
        assert_eq!(i.onde_acaba(130), inicios[192]);
        assert_eq!(i.onde_acaba(199), texto.len() as u64);
    }

    #[test]
    fn quebra_dentro_de_aspas_nao_cria_registro() {
        let v = indice(b"a;\"x\ny\r\nz\"\nb;c\n", b';');
        assert_eq!(v.indice.registros, 2);
    }

    #[test]
    fn utf8_invalido_e_comeco_depois_do_bom() {
        assert!(!indice(b"a;\xE7\n", b';').utf8_valido);
        let texto = b"\xEF\xBB\xBFa;b\n";
        let mut sem_aviso = |_: u64, _: u64| Ok(());
        let v = construir_indice(Cursor::new(&texto[3..]), Path::new("x"), 3, texto.len() as u64, b';', &mut sem_aviso)
            .unwrap();
        assert_eq!(v.indice.pontos(), &[3]);
        assert_eq!(v.indice.inicio, 3);
        assert!(v.utf8_valido);
    }

    #[test]
    fn progresso_e_cancelamento() {
        let mut texto = Vec::new();
        for i in 0..400_000 {
            texto.extend_from_slice(format!("{i};linha com algum texto para encher {i}\n").as_bytes());
        }
        assert!(texto.len() > 12 << 20);
        let mut avisos = Vec::new();
        let mut ao = |b: u64, t: u64| {
            avisos.push((b, t));
            Ok(())
        };
        construir_indice(Cursor::new(&texto), Path::new("x"), 0, texto.len() as u64, b';', &mut ao).unwrap();
        assert!(avisos.len() >= 3, "{avisos:?}");
        assert_eq!(avisos.last().unwrap(), &(texto.len() as u64, texto.len() as u64));
        assert!(avisos.windows(2).all(|p| p[0].0 <= p[1].0));

        let mut chamadas = 0;
        let mut cancela = |_: u64, _: u64| {
            chamadas += 1;
            Err(comum::ErroBanco::novo("cancelado").status(409))
        };
        let e = construir_indice(Cursor::new(&texto), Path::new("x"), 0, texto.len() as u64, b';', &mut cancela)
            .err()
            .unwrap();
        assert_eq!((e.mensagem.as_str(), e.status), ("cancelado", 409));
        assert_eq!(chamadas, 1, "para no primeiro aviso");
    }
}
