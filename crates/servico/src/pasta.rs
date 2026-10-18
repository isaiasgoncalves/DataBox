//! A pasta no disco (etapa 4): o que um caminho é, abrir e fechar a pasta do Explorador, ler uma pasta de cada
//! vez e registrar um arquivo de dados nos recentes. Todo o acesso a disco da árvore mora aqui; quem chama é
//! sempre uma tarefa de fundo (a leitura de uma pasta de rede pode demorar).
//!
//! As regras (decisões 1 a 5 do plano da etapa 4) usam as peças puras de [`comum::arquivos`]:
//!
//! - **O que um arquivo é**: extensão de dados conhecida vale sem ler; arquivo de 0 byte com extensão de banco é
//!   banco; o resto, com 100 bytes ou mais, tem os 16 primeiros bytes lidos (`SQLite format 3\0`).
//! - **Ocultos**: atributo oculto ou de sistema do Windows, os `~$` do Office e as pastas de ferramentas. Um ponto
//!   no começo do nome não esconde. Os arquivos de outros tipos (e os ocultos) só entram na conta de `ocultas`.
//! - **Atalhos de pasta** (link simbólico e junção) aparecem como pasta, com o tamanho e a data do destino; os
//!   quebrados saem. A leitura é de **um nível por vez** e nunca desce sozinha: um atalho que volta para uma pasta
//!   de cima não faz laço, só a pessoa que o abrir de novo anda em círculo (e a chave de cada nó leva o caminho
//!   inteiro).
//! - **Limites**: 500 entradas por pasta (o resto vira `a_mais`), 20.000 entradas lidas e 5 s por varredura, o
//!   cabeçalho lido em no máximo 2.000 arquivos; o que passa disso marca `parcial`. Os candidatos a cabeçalho
//!   são lidos primeiro os de extensão de banco, depois os demais em ordem natural, para o corte não esconder um
//!   `.db`. Um arquivo que o Windows guarda só na nuvem (OneDrive, `RECALL_ON_DATA_ACCESS`) não é aberto para
//!   ler o cabeçalho: abrir faria o Windows baixá-lo inteiro só para listar a pasta.
//!
//! Nada aqui toca na conexão do banco aberto nem em nenhuma trava do [`Servico`] além das preferências:
//! listar a pasta onde o banco está aberto responde na hora mesmo com uma Consulta SQL segurando a conexão.

use std::collections::HashMap;
use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use comum::arquivos::{
    Entrada, LIMITE_DE_CABECALHOS, LIMITE_DE_LEITURA, LIMITE_POR_PASTA, Listagem, Natureza, PRAZO_DA_VARREDURA,
    Palpite, TipoDeArquivo, atributos_de, comparar_nomes, e_cabecalho_sqlite, e_extensao_de_banco, e_oculto, palpite,
};
use comum::{ErroBanco, Resultado, traduzir_io};
use prefs::{Mudanca, TipoRecente, chave_caminho};

use crate::caminhos::{limpar_caminho, resolver};
use crate::servico::Servico;

/// `FILE_ATTRIBUTE_OFFLINE`, `FILE_ATTRIBUTE_RECALL_ON_OPEN` e `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`: o conteúdo
/// do arquivo não está no disco, e abri-lo o traria da nuvem.
const ATRIBUTOS_NA_NUVEM: u32 = 0x1000 | 0x4_0000 | 0x40_0000;

/// Os bytes do começo que bastam para reconhecer o SQLite.
const BYTES_DO_CABECALHO: u64 = 16;

/// O arquivo de dados que vai abrir (no editor do tipo, ou na ficha no build sem o recurso `telas`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoArquivo {
    pub caminho: PathBuf,
    pub nome: String,
    pub tipo: TipoDeArquivo,
    pub tamanho: u64,
    pub modificado_em: Option<SystemTime>,
}

/// Os limites de uma varredura. Os de verdade são [`Limites::PADRAO`]; os testes os reduzem.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limites {
    pub(crate) por_pasta: usize,
    pub(crate) leitura: usize,
    pub(crate) cabecalhos: usize,
    pub(crate) prazo: Duration,
}

impl Limites {
    pub(crate) const PADRAO: Limites = Limites {
        por_pasta: LIMITE_POR_PASTA,
        leitura: LIMITE_DE_LEITURA,
        cabecalhos: LIMITE_DE_CABECALHOS,
        prazo: PRAZO_DA_VARREDURA,
    };
}

/// O que uma varredura devolve: a listagem e quantos cabeçalhos foram lidos no disco (os reaproveitados do
/// `anterior` não contam), para os testes conferirem o limite e o reaproveitamento.
#[derive(Debug)]
pub(crate) struct Varredura {
    pub(crate) listagem: Listagem,
    /// Só os testes leem.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) cabecalhos_lidos: usize,
}

impl Servico {
    /// O que o caminho é: pasta, banco (pelo cabeçalho do arquivo, ou 0 byte com extensão de banco) ou arquivo de
    /// dados (CSV, Excel, JSON, texto). O caminho é limpo como os demais (aspas, `~`). Inexistente → 404 "O
    /// arquivo não existe (ou foi movido): …."; tipo que o app não abre → 400 "O DataBox ainda não abre este tipo
    /// de arquivo: relatorio.pdf.". Só lê o disco, nunca abre o banco.
    pub fn alvo_do_caminho(&self, caminho: &Path) -> Resultado<Natureza> {
        classificar(&limpar_caminho(caminho, "o caminho da pasta ou do arquivo")?)
    }

    /// Confere que é uma pasta, lê a raiz, a põe nos recentes (tipo Pasta) e grava `pasta_aberta` nas
    /// preferências. Não existe → 404 "A pasta não existe (ou foi movida): …."; é arquivo → 400 "O caminho não é
    /// uma pasta: …."; não deu para ler → a frase de `traduzir_io`, e nada é gravado.
    ///
    /// O caminho volta resolvido na `Listagem::pasta` (absoluto, com os links e os nomes curtos 8.3 resolvidos e
    /// sem o `\\?\`), como o do banco aberto: a casca compara os dois pela chave do caminho.
    pub fn abrir_pasta(&self, caminho: &Path) -> Resultado<Listagem> {
        let caminho = resolvida(&limpar_caminho(caminho, "o caminho da pasta")?);
        exigir_pasta(&caminho)?;
        let listagem = varrer(&caminho, None, Limites::PADRAO)?.listagem;
        self.prefs.adicionar_recente_de(&caminho, TipoRecente::Pasta);
        if let Err(e) = self.prefs.atualizar(Mudanca::pasta_aberta(Some(caminho.clone()))) {
            log::warn!("Pasta aberta não guardada ({}): {e}", caminho.display());
        }
        Ok(listagem)
    }

    /// Lê uma pasta (a raiz, uma subpasta que abriu, o Atualizar): um nível, ordenado (pastas, bancos, dados),
    /// cortado em 500. `anterior` (a leitura anterior da mesma pasta) reaproveita o cabeçalho dos bancos cujo
    /// tamanho e data não mudaram. Falha de leitura: a frase de `traduzir_io`; o caminho de um arquivo → 400
    /// "O caminho não é uma pasta: ….". Não grava nada. O caminho fica como veio (um atalho de pasta continua o
    /// atalho, e as entradas dele também): só o [`Servico::abrir_pasta`] resolve a raiz.
    pub fn listar_pasta(&self, caminho: &Path, anterior: Option<&Listagem>) -> Resultado<Listagem> {
        let caminho = limpar_caminho(caminho, "o caminho da pasta")?;
        Ok(varrer(&caminho, anterior, Limites::PADRAO)?.listagem)
    }

    /// O Fechar pasta: apaga `pasta_aberta` das preferências (a pasta não volta na próxima abertura). Os
    /// recentes ficam. Erro de gravação só vai para o registro.
    pub fn fechar_pasta(&self) {
        if let Err(e) = self.prefs.atualizar(Mudanca::pasta_aberta(None)) {
            log::warn!("Pasta aberta não apagada: {e}");
        }
    }

    /// O arquivo de dados que vai abrir: confere o tipo, entra nos recentes (tipo Arquivo) e devolve o que a
    /// ficha mostra. Inexistente → 404; pasta ou banco → 400 "O caminho não é um arquivo de dados: …."; tipo
    /// que o app não abre → 400 como em [`Servico::alvo_do_caminho`].
    pub fn registrar_arquivo(&self, caminho: &Path) -> Resultado<InfoArquivo> {
        let info = self.info_arquivo(caminho)?;
        self.prefs.adicionar_recente_de(&info.caminho, TipoRecente::Arquivo);
        Ok(info)
    }

    /// O que a ficha de um arquivo de dados mostra (o tipo, o tamanho e a data), sem mexer nos recentes. Os erros
    /// são os de [`Servico::registrar_arquivo`]. O caminho volta resolvido, como o da pasta e o do banco.
    pub fn info_arquivo(&self, caminho: &Path) -> Resultado<InfoArquivo> {
        let caminho = resolvida(&limpar_caminho(caminho, "o caminho do arquivo")?);
        let tipo = match classificar(&caminho)? {
            Natureza::Dados(tipo) => tipo,
            Natureza::Pasta | Natureza::Banco => {
                return Err(ErroBanco::novo(format!("O caminho não é um arquivo de dados: {}.", caminho.display())));
            }
        };
        let meta = fs::metadata(&caminho).map_err(|e| erro_de_acesso(&e, &caminho))?;
        Ok(InfoArquivo {
            nome: nome_de(&caminho),
            caminho,
            tipo,
            tamanho: meta.len(),
            modificado_em: meta.modified().ok(),
        })
    }
}

/// A pasta resolvida (ver [`Servico::abrir_pasta`]); se não der para resolver, como veio (a leitura diz o erro).
fn resolvida(caminho: &Path) -> PathBuf {
    resolver(caminho).unwrap_or_else(|_| caminho.to_path_buf())
}

fn nome_de(caminho: &Path) -> String {
    caminho.file_name().map_or_else(|| caminho.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// 404 "O arquivo não existe (ou foi movido): …." para o que não existe; o resto, a frase de `traduzir_io`.
fn erro_de_acesso(e: &io::Error, caminho: &Path) -> ErroBanco {
    if matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::NotADirectory) {
        return ErroBanco::com_status(format!("O arquivo não existe (ou foi movido): {}.", caminho.display()), 404)
            .detalhe(e.to_string());
    }
    traduzir_io(e, Some(caminho), None)
}

fn nao_abre(nome: &str) -> ErroBanco {
    ErroBanco::novo(format!("O DataBox ainda não abre este tipo de arquivo: {nome}."))
}

/// O que o caminho (já limpo) é. Segue atalhos, como o Explorador do Windows.
fn classificar(caminho: &Path) -> Resultado<Natureza> {
    let meta = fs::metadata(caminho).map_err(|e| erro_de_acesso(&e, caminho))?;
    if meta.is_dir() {
        return Ok(Natureza::Pasta);
    }
    let nome = nome_de(caminho);
    if !meta.is_file() {
        return Err(nao_abre(&nome));
    }
    match palpite(&nome, meta.len()) {
        Palpite::Dados(tipo) => Ok(Natureza::Dados(tipo)),
        Palpite::BancoVazio => Ok(Natureza::Banco),
        Palpite::LerCabecalho => {
            if ler_cabecalho(caminho).map_err(|e| traduzir_io(&e, Some(caminho), None))? {
                Ok(Natureza::Banco)
            } else {
                Err(nao_abre(&nome))
            }
        }
        Palpite::Nenhuma => Err(nao_abre(&nome)),
    }
}

/// 404 "A pasta não existe (ou foi movida): …." / 400 "O caminho não é uma pasta: …."
fn exigir_pasta(caminho: &Path) -> Resultado<()> {
    match fs::metadata(caminho) {
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(ErroBanco::novo(format!("O caminho não é uma pasta: {}.", caminho.display()))),
        Err(e) if matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::NotADirectory) => {
            Err(ErroBanco::com_status(format!("A pasta não existe (ou foi movida): {}.", caminho.display()), 404)
                .detalhe(e.to_string()))
        }
        Err(e) => Err(traduzir_io(&e, Some(caminho), None)),
    }
}

/// Os 16 primeiros bytes do arquivo são o cabeçalho do SQLite?
fn ler_cabecalho(caminho: &Path) -> io::Result<bool> {
    let mut inicio = Vec::with_capacity(BYTES_DO_CABECALHO as usize);
    File::open(caminho)?.take(BYTES_DO_CABECALHO).read_to_end(&mut inicio)?;
    Ok(e_cabecalho_sqlite(&inicio))
}

/// Um arquivo que pode ser banco e ainda espera a leitura do cabeçalho.
struct Candidato {
    nome: String,
    caminho: PathBuf,
    tamanho: u64,
    modificado: Option<SystemTime>,
    /// A extensão é de banco: esses são lidos primeiro.
    extensao_de_banco: bool,
    /// O conteúdo está na nuvem: não se abre para listar.
    na_nuvem: bool,
}

/// Uma entrada ainda sem a `chave` (só as que sobram do corte a recebem).
fn entrada(
    nome: String,
    caminho: PathBuf,
    natureza: Natureza,
    tamanho: Option<u64>,
    modificado: Option<SystemTime>,
) -> Entrada {
    Entrada { nome, caminho, chave: String::new(), natureza, tamanho, modificado }
}

/// A leitura de uma pasta (ver a documentação do módulo).
pub(crate) fn varrer(pasta: &Path, anterior: Option<&Listagem>, limites: Limites) -> Resultado<Varredura> {
    let inicio = Instant::now();
    let meta = fs::metadata(pasta).map_err(|e| traduzir_io(&e, Some(pasta), None))?;
    if !meta.is_dir() {
        return Err(ErroBanco::novo(format!("O caminho não é uma pasta: {}.", pasta.display())));
    }
    let leitor = fs::read_dir(pasta).map_err(|e| traduzir_io(&e, Some(pasta), None))?;
    // os bancos da leitura anterior da mesma pasta, pelo nome sem caixa
    let anteriores: HashMap<String, &Entrada> = match anterior {
        Some(a) if chave_caminho(&a.pasta) == chave_caminho(pasta) => {
            a.entradas.iter().filter(|e| e.natureza == Natureza::Banco).map(|e| (e.nome.to_lowercase(), e)).collect()
        }
        _ => HashMap::new(),
    };

    let mut entradas: Vec<Entrada> = Vec::new();
    let mut candidatos: Vec<Candidato> = Vec::new();
    let (mut ocultas, mut parcial, mut lidas) = (0usize, false, 0usize);
    for item in leitor {
        let Ok(item) = item else {
            parcial = true;
            break;
        };
        if lidas >= limites.leitura || inicio.elapsed() >= limites.prazo {
            parcial = true;
            break;
        }
        lidas += 1;
        let nome = item.file_name().to_string_lossy().into_owned();
        let caminho = item.path();
        let Ok(proprio) = item.metadata() else {
            ocultas += 1;
            continue;
        };
        let atributos = atributos_de(&proprio);
        let meta: Metadata = if proprio.file_type().is_symlink() {
            match fs::metadata(&caminho) {
                Ok(destino) => destino,
                Err(_) => continue, // atalho quebrado
            }
        } else {
            proprio
        };
        let (e_pasta, e_arquivo) = (meta.is_dir(), meta.is_file());
        if !e_pasta && !e_arquivo {
            continue;
        }
        if e_oculto(&nome, e_pasta, atributos) {
            ocultas += usize::from(e_arquivo);
            continue;
        }
        if e_pasta {
            // sem a data: o NTFS só atualiza a data de uma subpasta, na leitura da pasta de cima, depois que alguém
            // a abre; com ela, a releitura da volta do foco acharia mudança onde não há (e repintaria à toa)
            entradas.push(entrada(nome, caminho, Natureza::Pasta, None, None));
            continue;
        }
        let modificado = meta.modified().ok();
        let tamanho = meta.len();
        match palpite(&nome, tamanho) {
            Palpite::Dados(tipo) => {
                entradas.push(entrada(nome, caminho, Natureza::Dados(tipo), Some(tamanho), modificado));
            }
            Palpite::BancoVazio => entradas.push(entrada(nome, caminho, Natureza::Banco, Some(tamanho), modificado)),
            Palpite::LerCabecalho => {
                let extensao_de_banco =
                    Path::new(&nome).extension().and_then(|e| e.to_str()).is_some_and(e_extensao_de_banco);
                candidatos.push(Candidato {
                    nome,
                    caminho,
                    tamanho,
                    modificado,
                    extensao_de_banco,
                    na_nuvem: atributos & ATRIBUTOS_NA_NUVEM != 0,
                });
            }
            Palpite::Nenhuma => ocultas += 1,
        }
    }

    // primeiro os de extensão de banco, depois os demais em ordem natural: o corte dos cabeçalhos nunca esconde
    // um `.db` por causa de milhares de arquivos sem extensão
    candidatos.sort_by(|a, b| {
        (!a.extensao_de_banco).cmp(&!b.extensao_de_banco).then_with(|| comparar_nomes(&a.nome, &b.nome))
    });
    let mut cabecalhos_lidos = 0usize;
    for c in candidatos {
        let igual = anteriores
            .get(&c.nome.to_lowercase())
            .is_some_and(|e| c.modificado.is_some() && e.tamanho == Some(c.tamanho) && e.modificado == c.modificado);
        if igual {
            entradas.push(entrada(c.nome, c.caminho, Natureza::Banco, Some(c.tamanho), c.modificado));
            continue;
        }
        if c.na_nuvem {
            ocultas += 1;
            continue;
        }
        if cabecalhos_lidos >= limites.cabecalhos || inicio.elapsed() >= limites.prazo {
            parcial = true; // fica de fora: não se sabe o que é
            continue;
        }
        cabecalhos_lidos += 1;
        match ler_cabecalho(&c.caminho) {
            Ok(true) => entradas.push(entrada(c.nome, c.caminho, Natureza::Banco, Some(c.tamanho), c.modificado)),
            Ok(false) => ocultas += 1,
            Err(e) => {
                log::debug!("Cabeçalho de {} não lido: {e}", c.caminho.display());
                ocultas += 1;
            }
        }
    }

    let mut listagem = Listagem::montar(pasta.to_path_buf(), entradas, ocultas, parcial, limites.por_pasta);
    for e in &mut listagem.entradas {
        e.chave = chave_caminho(&e.caminho);
    }
    Ok(Varredura { listagem, cabecalhos_lidos })
}
