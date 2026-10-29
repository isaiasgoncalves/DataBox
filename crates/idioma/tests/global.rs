//! O idioma global, num binário de teste só dele (um processo à parte: os outros testes não o veem mudar).

use idioma::{INGLES, PORTUGUES, atual, com_idioma, definir, t};

#[test]
fn definir_vale_para_todas_as_threads_menos_as_isoladas() {
    assert_eq!(atual(), PORTUGUES, "antes de qualquer definir, o português");
    definir(INGLES);
    assert_eq!(std::thread::spawn(atual).join().unwrap(), INGLES);
    assert_eq!(std::thread::spawn(|| t!("Abrir pasta").into_owned()).join().unwrap(), "Open folder");
    // a thread isolada não vê o global e o definir dela não sai dela
    com_idioma(PORTUGUES, || {
        assert_eq!(t!("Abrir pasta"), "Abrir pasta");
        definir(INGLES);
        definir(PORTUGUES);
        assert_eq!(std::thread::spawn(atual).join().unwrap(), INGLES);
    });
    assert_eq!(atual(), INGLES);
    definir(PORTUGUES);
    assert_eq!(std::thread::spawn(atual).join().unwrap(), PORTUGUES);
}
