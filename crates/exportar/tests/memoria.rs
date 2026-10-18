//! Pico de memória da exportação em fluxo : 20.000 linhas de 1 KB (uns 20 MB se ficassem na memória) exportadas em cada
//! formato com pico bem menor.
//!
//! Fica num teste de integração (crate à parte) porque contar as alocações exige um alocador global, e o
//! `lib.rs` proíbe `unsafe`. O único `unsafe` aqui repassa cada chamada ao alocador do sistema como veio.

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Mutex, MutexGuard};

use comum::{ErroBanco, Valor};
use exportar::{Acompanhamento, Formato, Opcoes, exportar_linhas, exportar_varias};

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

fn linhas() -> impl Iterator<Item = Result<Vec<Valor>, ErroBanco>> {
    (0..20_000i64).map(|i| Ok(vec![Valor::Inteiro(i), Valor::Texto(format!("{i:06}{}", "x".repeat(1000)))]))
}

/// Pico de memória (acima do que já estava alocado) enquanto `f` roda. Os contadores são do processo
/// inteiro: um teste que mede de cada vez (ver [`um_de_cada_vez`]).
fn pico_de(f: impl FnOnce()) -> isize {
    let base = ATUAL.load(Ordering::SeqCst);
    PICO.store(base, Ordering::SeqCst);
    f();
    PICO.load(Ordering::SeqCst) - base
}

/// Os testes rodam em paralelo e as alocações de um entrariam na conta do outro.
fn um_de_cada_vez() -> MutexGuard<'static, ()> {
    static VEZ: Mutex<()> = Mutex::new(());
    VEZ.lock().unwrap_or_else(|e| e.into_inner())
}

/// A amostra das larguras do Excel (as 200 primeiras linhas de cada fonte) guarda só o que vai para a
/// célula: 200 linhas com um BLOB de 1 MB e um texto de 300 mil caracteres (uns 260 MB se a amostra guardasse
/// os valores inteiros; com anexos de 20 MB seriam 4 GB, e falta de memória no Rust derruba o processo) ficam
/// em uns 8 MB: o marcador do BLOB e os 32.767 caracteres que cabem na célula.
#[test]
fn amostra_do_excel_nao_guarda_os_valores_inteiros() {
    let _vez = um_de_cada_vez();
    let pasta: PathBuf = std::env::temp_dir().join(format!("exportar-teste-amostra-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&pasta);
    std::fs::create_dir_all(&pasta).unwrap();
    let colunas = vec!["n".to_string(), "anexo".to_string(), "texto".to_string()];
    let destino = pasta.join("anexos.xlsx");
    let mut resumo = None;
    let pico = pico_de(|| {
        let linhas = (0..200i64).map(|i| {
            Ok::<_, ErroBanco>(vec![
                Valor::Inteiro(i),
                Valor::Blob(vec![(i % 251) as u8; 1_000_000]),
                Valor::Texto("y".repeat(300_000)),
            ])
        });
        resumo = Some(
            exportar_linhas(
                &colunas,
                linhas,
                Formato::Xlsx,
                &destino,
                &Opcoes::default(),
                "anexos",
                Acompanhamento::default(),
            )
            .unwrap(),
        );
    });
    let resumo = resumo.unwrap();
    eprintln!("xlsx (amostra com anexos): pico de {:.2} MB", pico as f64 / 1024.0 / 1024.0);
    assert_eq!(resumo.linhas, 200);
    assert_eq!(resumo.avisos.len(), 2, "{:?}", resumo.avisos); // 200 textos cortados e 200 BLOBs trocados
    assert!(resumo.avisos[0].starts_with("200 células de texto passaram"), "{:?}", resumo.avisos);
    assert!(pico < 16 * 1024 * 1024, "pico de {:.1} MB", pico as f64 / 1024.0 / 1024.0);
    std::fs::remove_dir_all(&pasta).unwrap();
}

#[test]
fn exportacao_em_fluxo_nao_materializa() {
    let _vez = um_de_cada_vez();
    let pasta: PathBuf = std::env::temp_dir().join(format!("exportar-teste-memoria-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&pasta);
    std::fs::create_dir_all(&pasta).unwrap();
    let colunas = vec!["n".to_string(), "texto".to_string()];
    for formato in Formato::TODOS {
        let destino = pasta.join(format!("fluxo.{formato}"));
        let mut linhas_gravadas = 0;
        let pico = pico_de(|| {
            let resumo = exportar_linhas(
                &colunas,
                linhas(),
                formato,
                &destino,
                &Opcoes::default(),
                "dados",
                Acompanhamento::default(),
            )
            .unwrap();
            linhas_gravadas = resumo.linhas;
        });
        assert_eq!(linhas_gravadas, 20_000);
        eprintln!("{formato}: pico de {:.2} MB", pico as f64 / 1024.0 / 1024.0);
        assert!(pico < 8 * 1024 * 1024, "{formato}: pico de {:.1} MB", pico as f64 / 1024.0 / 1024.0);

        // o banco inteiro (no CSV, o .zip com um CSV por tabela)
        let destino = pasta.join(format!("banco.{formato}"));
        let pico = pico_de(|| {
            let fontes = vec![("a", colunas.clone(), linhas()), ("b", colunas.clone(), linhas())];
            let resumo =
                exportar_varias(fontes, formato, &destino, &Opcoes::default(), None, Acompanhamento::default())
                    .unwrap();
            assert_eq!(resumo.linhas, 40_000);
        });
        eprintln!("{formato} (banco): pico de {:.2} MB", pico as f64 / 1024.0 / 1024.0);
        assert!(pico < 8 * 1024 * 1024, "{formato} (banco): pico de {:.1} MB", pico as f64 / 1024.0 / 1024.0);
    }
    std::fs::remove_dir_all(&pasta).unwrap();
}

/// Tempo e pico de 1 milhão de linhas em cada formato (medição, não teste: rode com `--release -- --ignored
/// --nocapture um_milhao`).
#[test]
#[ignore]
fn um_milhao_de_linhas() {
    let _vez = um_de_cada_vez();
    let pasta: PathBuf = std::env::temp_dir().join(format!("exportar-medida-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&pasta);
    std::fs::create_dir_all(&pasta).unwrap();
    let colunas: Vec<String> =
        ["id", "nome", "valor", "cidade", "ativo", "data"].iter().map(|s| s.to_string()).collect();
    for formato in Formato::TODOS {
        let destino = pasta.join(format!("milhao.{formato}"));
        let inicio = std::time::Instant::now();
        let mut bytes = 0;
        let pico = pico_de(|| {
            let linhas = (0..1_000_000i64).map(|i| {
                Ok::<_, ErroBanco>(vec![
                    Valor::Inteiro(i),
                    Valor::Texto(format!("Cliente {i}")),
                    Valor::Real(i as f64 * 1.25),
                    if i % 9 == 0 { Valor::Nulo } else { Valor::Texto("São Paulo".into()) },
                    Valor::Inteiro(i % 2),
                    Valor::Texto(format!("2026-09-{:02}", i % 28 + 1)),
                ])
            });
            let resumo = exportar_linhas(
                &colunas,
                linhas,
                formato,
                &destino,
                &Opcoes::default(),
                "dados",
                Acompanhamento::default(),
            )
            .unwrap();
            bytes = resumo.bytes;
        });
        eprintln!(
            "{formato}: 1.000.000 linhas x 6 colunas em {:.2} s, {:.1} MB no disco, pico de {:.2} MB",
            inicio.elapsed().as_secs_f64(),
            bytes as f64 / 1024.0 / 1024.0,
            pico as f64 / 1024.0 / 1024.0
        );
    }
    std::fs::remove_dir_all(&pasta).unwrap();
}
