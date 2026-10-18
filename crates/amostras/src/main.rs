//! `cargo run -p amostras -- destino.db`: cria (ou recria) o banco de exemplo.

fn main() {
    let destino = std::env::args().nth(1).unwrap_or_else(|| "exemplo.db".into());
    match amostras::criar_banco_exemplo(&destino) {
        Ok(c) => println!("Banco de exemplo criado em {}", c.display()),
        Err(e) => {
            eprintln!("Não foi possível criar {destino}: {e}");
            std::process::exit(1);
        }
    }
}
