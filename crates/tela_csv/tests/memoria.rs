//! Pico de memória das contas da coluna (a seção Coluna das Propriedades): a coluna passa em fluxo
//! (`tabular::csv::percorrer_coluna`) pelas `Contas`, um valor por vez, e o pico não cresce com o tamanho dela. Juntar
//! a coluna num `Vec<String>` (o que se fazia antes) custaria o tamanho da coluna inteira.
//!
//! Fica num teste de integração (crate à parte) porque contar as alocações exige um alocador global, e o `lib.rs`
//! proíbe `unsafe`. O único `unsafe` aqui repassa cada chamada ao alocador do sistema como veio (o mesmo do
//! `exportar/tests/memoria.rs`).

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]
// o alocador que conta (abaixo) é o único `unsafe` liberado fora da FFI do SQLite
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Mutex, MutexGuard};

use comum::Resultado;
use tabular::csv::{OpcoesAbertura, Tabela, percorrer_coluna};
use tabular::numero::TipoDetectado;
use tela_csv::{Contas, Estatistica, calcular_estatistica};

struct Contador;

static ATUAL: AtomicIsize = AtomicIsize::new(0);
static PICO: AtomicIsize = AtomicIsize::new(0);

fn somar(delta: isize) {
    let atual = ATUAL.fetch_add(delta, Ordering::SeqCst) + delta;
    PICO.fetch_max(atual, Ordering::SeqCst);
}

// SAFETY: cada método só repassa os mesmos argumentos ao `System` (que cumpre o contrato do GlobalAlloc) e
// soma o tamanho num contador atômico; nenhum ponteiro é lido ou alterado aqui.
unsafe impl GlobalAlloc for Contador {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: o chamador cumpre o contrato de `alloc`, que é repassado sem mudança.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            somar(layout.size() as isize);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` veio deste alocador (que é o `System`) com este `layout`.
        unsafe { System.dealloc(ptr, layout) };
        somar(-(layout.size() as isize));
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, novo: usize) -> *mut u8 {
        // SAFETY: `ptr` veio deste alocador com este `layout`; o contrato de `realloc` é repassado.
        let p = unsafe { System.realloc(ptr, layout, novo) };
        if !p.is_null() {
            somar(novo as isize - layout.size() as isize);
        }
        p
    }
}

#[global_allocator]
static ALOCADOR: Contador = Contador;

const MB: f64 = 1024.0 * 1024.0;

/// Pico de memória (acima do que já estava alocado) enquanto `f` roda. Os contadores são do processo
/// inteiro: um teste que mede de cada vez (ver [`um_de_cada_vez`]).
fn pico_de<T>(f: impl FnOnce() -> T) -> (T, isize) {
    let base = ATUAL.load(Ordering::SeqCst);
    PICO.store(base, Ordering::SeqCst);
    let r = f();
    (r, PICO.load(Ordering::SeqCst) - base)
}

/// Os testes rodam em paralelo e as alocações de um entrariam na conta do outro.
fn um_de_cada_vez() -> MutexGuard<'static, ()> {
    static VEZ: Mutex<()> = Mutex::new(());
    VEZ.lock().unwrap_or_else(|e| e.into_inner())
}

/// Um CSV de `linhas` linhas com um número e um texto de 1 KB (uns `linhas` KB no disco).
fn gravar(pasta: &Path, linhas: u64) -> PathBuf {
    let caminho = pasta.join(format!("t{linhas}.csv"));
    let mut f = std::io::BufWriter::new(std::fs::File::create(&caminho).unwrap());
    writeln!(f, "n;texto").unwrap();
    let texto = "x".repeat(1000);
    for i in 0..linhas {
        writeln!(f, "{i};{i:06}{texto}").unwrap();
    }
    f.flush().unwrap();
    caminho
}

fn sem_progresso(_: u64, _: u64) -> Resultado<()> {
    Ok(())
}

/// As contas da coluna `c` em fluxo, como a tela faz (um leitor próprio e as alterações da tabela).
fn contas_em_fluxo(t: &Tabela, c: usize, tipo: TipoDetectado) -> Estatistica {
    let mut leitor = t.leitor().reabrir_para_leitura().unwrap();
    let mut contas = Contas::nova(tipo, ',');
    percorrer_coluna(&mut leitor, t.edicoes(), c, &mut sem_progresso, |v| {
        contas.somar(v);
        Ok(true)
    })
    .unwrap();
    contas.estatistica()
}

#[test]
fn as_contas_da_coluna_nao_juntam_a_coluna() {
    let _vez = um_de_cada_vez();
    let pasta = std::env::temp_dir().join(format!("tela_csv-teste-memoria-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&pasta);
    std::fs::create_dir_all(&pasta).unwrap();
    let mut picos = Vec::new();
    for linhas in [10_000u64, 40_000] {
        let mut t = Tabela::abrir(&gravar(&pasta, linhas), &OpcoesAbertura::default()).unwrap();
        // com alterações por cima: uma célula editada e uma linha nova no fim
        t.editar_celula(0, 1, "curto").unwrap();
        t.acrescentar_linhas(1).unwrap();

        let (texto, pico_texto) = pico_de(|| contas_em_fluxo(&t, 1, TipoDetectado::Texto));
        assert_eq!((texto.vazias, texto.maior_texto), (1, Some(1006)), "{linhas} linhas");
        let (numeros, pico_numeros) = pico_de(|| contas_em_fluxo(&t, 0, TipoDetectado::Inteiro));
        let soma: u64 = (0..linhas).sum();
        assert_eq!(numeros.vazias, 1);
        assert_eq!(numeros.minimo.as_deref(), Some("0"));
        assert_eq!(numeros.maximo.as_deref(), Some((linhas - 1).to_string().as_str()));
        assert_eq!(numeros.soma.as_deref().map(|s| s.replace('.', "")), Some(soma.to_string()));

        // os mesmos números de quem junta a coluna inteira (o caminho de antes), que custa a coluna inteira
        let (juntas, pico_juntas) = pico_de(|| {
            let coluna = t.colunas_de(&[1], &mut sem_progresso).unwrap().into_iter().next().unwrap();
            calcular_estatistica(coluna, TipoDetectado::Texto, ',')
        });
        assert_eq!(juntas, texto);

        let pico = pico_texto.max(pico_numeros);
        eprintln!(
            "{linhas} linhas ({:.1} MB na coluna): pico de {:.2} MB em fluxo, {:.1} MB juntando a coluna",
            linhas as f64 * 1007.0 / MB,
            pico as f64 / MB,
            pico_juntas as f64 / MB
        );
        assert!(pico_juntas as f64 > linhas as f64 * 1000.0, "juntar a coluna custa a coluna inteira");
        assert!(pico < 4 * 1024 * 1024, "{linhas} linhas: pico de {:.2} MB", pico as f64 / MB);
        picos.push(pico);
    }
    // 4 vezes a coluna, o mesmo pico (com folga para o que o alocador arredonda)
    let (menor, maior) = (picos[0], picos[1]);
    assert!(maior < menor + 256 * 1024, "o pico cresceu com a coluna: {menor} → {maior} bytes");
    std::fs::remove_dir_all(&pasta).unwrap();
}
