//! O conteúdo descomprimido de uma parte do zip como um `Read` que pode ficar guardado: uma linha de
//! execução abre o arquivo, descomprime em blocos de 256 KB e entrega por um canal de pouca profundidade.
//!
//! O `ZipFile` do crate `zip` empresta o `ZipArchive` (não dá para guardá-lo numa estrutura ao lado do
//! leitor, em Rust seguro), e é por isso que a descompressão mora numa linha própria: quem lê o XML fica com um
//! objeto comum, sem empréstimo, que continua de onde parou na chamada seguinte. De quebra, descomprimir e
//! interpretar o XML andam em paralelo. A memória é limitada pelo canal (1 MB). Ao soltar o [`Fluxo`], o
//! canal fecha, a linha termina e é esperada (o arquivo não fica aberto por trás).

use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread::JoinHandle;

const BLOCO: usize = 256 * 1024;
const BLOCOS_NO_CANAL: usize = 4;

type Pedaco = io::Result<Vec<u8>>;

pub(crate) struct Fluxo {
    canal: Option<Receiver<Pedaco>>,
    atual: Vec<u8>,
    pos: usize,
    linha: Option<JoinHandle<()>>,
    /// Falha ao criar a linha de execução, devolvida na primeira leitura.
    falha: Option<io::Error>,
}

impl Fluxo {
    /// Começa a descomprimir `parte` do zip em `caminho`.
    pub fn abrir(caminho: PathBuf, parte: String) -> Fluxo {
        let (envio, canal) = sync_channel(BLOCOS_NO_CANAL);
        let criada = std::thread::Builder::new()
            .name("planilha-descompressao".to_string())
            .spawn(move || corpo(&caminho, &parte, &envio));
        match criada {
            Ok(linha) => Fluxo { canal: Some(canal), atual: Vec::new(), pos: 0, linha: Some(linha), falha: None },
            Err(e) => Fluxo { canal: None, atual: Vec::new(), pos: 0, linha: None, falha: Some(e) },
        }
    }
}

fn corpo(caminho: &PathBuf, parte: &str, envio: &SyncSender<Pedaco>) {
    let resultado = (|| -> io::Result<()> {
        let arquivo = File::open(caminho)?;
        let mut zip = zip::ZipArchive::new(arquivo).map_err(io::Error::other)?;
        let mut entrada = zip.by_name(parte).map_err(io::Error::other)?;
        loop {
            let mut bloco = Vec::with_capacity(BLOCO);
            (&mut entrada).take(BLOCO as u64).read_to_end(&mut bloco)?;
            if bloco.is_empty() {
                return Ok(());
            }
            let fim = bloco.len() < BLOCO;
            if envio.send(Ok(bloco)).is_err() {
                return Ok(()); // quem lia desistiu
            }
            if fim {
                return Ok(());
            }
        }
    })();
    if let Err(e) = resultado {
        let _ = envio.send(Err(e));
    }
}

impl Read for Fluxo {
    fn read(&mut self, saida: &mut [u8]) -> io::Result<usize> {
        if let Some(e) = self.falha.take() {
            return Err(e);
        }
        loop {
            if self.pos < self.atual.len() {
                let n = (self.atual.len() - self.pos).min(saida.len());
                saida[..n].copy_from_slice(&self.atual[self.pos..self.pos + n]);
                self.pos += n;
                return Ok(n);
            }
            let Some(canal) = &self.canal else {
                return Ok(0);
            };
            match canal.recv() {
                Ok(Ok(bloco)) => {
                    self.atual = bloco;
                    self.pos = 0;
                }
                Ok(Err(e)) => {
                    self.canal = None;
                    return Err(e);
                }
                Err(_) => {
                    self.canal = None;
                    return Ok(0);
                }
            }
        }
    }
}

impl Drop for Fluxo {
    fn drop(&mut self) {
        self.canal = None;
        if let Some(linha) = self.linha.take() {
            let _ = linha.join();
        }
    }
}
