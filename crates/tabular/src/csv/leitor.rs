//! A leitura de um CSV grande: abre, detecta o formato, monta o índice numa passada e depois lê só as páginas
//! pedidas (por posição, sem carregar o arquivo). Nada aqui altera o arquivo; as alterações vivem em
//! [`Edicoes`](super::Edicoes) e entram no disco em [`Tabela::salvar`](super::Tabela::salvar).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use comum::{ErroBanco, Resultado};

use super::analise::{BLOCO, Campo, Varredura, decodificar_campo, registros_do_trecho};
use super::arquivos::{CONTEXTO_LER, erro_io};
use super::codificacao::{BOM_UTF8, Codificacao};
use super::deteccao::{
    REGISTROS_DA_AMOSTRA, detectar_aspas, detectar_cabecalho, detectar_decimal, detectar_fim_de_linha,
    detectar_separador,
};
use super::formato::{Formato, OpcoesAbertura, decimal_valido, separador_valido};
use super::indice::{Indice, construir_indice};

/// O começo do arquivo que a detecção examina.
pub const TAMANHO_DA_AMOSTRA: usize = 256 << 10;

/// Um registro visto por dentro, durante [`Leitor::varrer`]: os campos são decodificados só quando pedidos.
pub struct VistaLinha<'a> {
    buf: &'a [u8],
    campos: &'a [Campo],
    cod: Codificacao,
}

impl VistaLinha<'_> {
    pub fn n_campos(&self) -> usize {
        self.campos.len()
    }

    /// O texto do campo `j` (vazio se o registro não tem tanto campo).
    pub fn campo(&self, j: usize) -> String {
        self.campos.get(j).map_or_else(String::new, |c| decodificar_campo(self.buf, *c, self.cod))
    }

    pub fn todos(&self) -> Vec<String> {
        self.campos.iter().map(|c| decodificar_campo(self.buf, *c, self.cod)).collect()
    }
}

/// Um CSV aberto para leitura, com o formato detectado e o índice dos registros.
#[derive(Debug)]
pub struct Leitor {
    caminho: PathBuf,
    arquivo: Option<File>,
    formato: Formato,
    indice: Arc<Indice>,
    modificado: Option<SystemTime>,
    nomes: Vec<String>,
    /// O texto do arquivo todo (depois do BOM) é UTF-8 válido. Lido como UTF-8 sem ser, as células trazem U+FFFD no
    /// lugar dos bytes que não são UTF-8, e a gravação recusa (regravaria a célula estragada).
    utf8_valido: bool,
}

fn sem_progresso(_: u64, _: u64) -> Resultado<()> {
    Ok(())
}

impl Leitor {
    /// Abre e indexa. Em arquivo grande leva tempo (uma passada pelo arquivo todo): para acompanhar ou
    /// cancelar, use [`Leitor::abrir_com_progresso`].
    pub fn abrir(caminho: &Path, opcoes: &OpcoesAbertura) -> Resultado<Leitor> {
        Leitor::abrir_com_progresso(caminho, opcoes, &mut sem_progresso)
    }

    /// Como [`Leitor::abrir`], com `ao_progresso(bytes_lidos, bytes_do_arquivo)` chamado de tempos em tempos.
    /// Se ele devolver um erro, a leitura para e o erro sobe como veio (é assim que se cancela).
    pub fn abrir_com_progresso(
        caminho: &Path,
        opcoes: &OpcoesAbertura,
        ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
    ) -> Resultado<Leitor> {
        let io = |e: std::io::Error| erro_io(&e, Some(caminho), CONTEXTO_LER);
        let mut arquivo = File::open(caminho).map_err(io)?;
        let meta = arquivo.metadata().map_err(io)?;
        if !meta.is_file() {
            return Err(ErroBanco::novo(format!("Isto não é um arquivo: {}.", caminho.display())));
        }
        let modificado = meta.modified().ok();
        let tamanho = meta.len();
        let n = usize::try_from(tamanho.min(TAMANHO_DA_AMOSTRA as u64)).unwrap_or(TAMANHO_DA_AMOSTRA);
        let mut amostra = vec![0u8; n];
        arquivo.read_exact(&mut amostra).map_err(io)?;
        if amostra.starts_with(&[0xFF, 0xFE]) || amostra.starts_with(&[0xFE, 0xFF]) {
            return Err(ErroBanco::novo(
                "O arquivo está em UTF-16, que o DataBox ainda não abre como tabela. Salve-o como UTF-8 no programa de origem.",
            ));
        }
        let bom = amostra.starts_with(&BOM_UTF8);
        let inicio: u64 = if bom && opcoes.codificacao.is_none_or(Codificacao::e_utf8) { 3 } else { 0 };
        let corpo = amostra.get(inicio as usize..).unwrap_or_default();
        if corpo.contains(&0) {
            return Err(ErroBanco::novo(
                "O arquivo não parece de texto (tem bytes nulos); o DataBox só abre CSV de texto.",
            ));
        }
        let truncada = tamanho > amostra.len() as u64;
        let separador = match opcoes.separador {
            Some(c) => {
                separador_valido(c)?;
                c
            }
            None => detectar_separador(corpo, truncada),
        };
        let sep = separador_valido(separador)?;
        let decimal_pedido = opcoes.decimal.map(decimal_valido).transpose()?;
        let fim_de_linha = opcoes.fim_de_linha.unwrap_or_else(|| detectar_fim_de_linha(corpo, separador, truncada));

        arquivo.seek(SeekFrom::Start(inicio)).map_err(io)?;
        let varrido = construir_indice(&mut arquivo, caminho, inicio, tamanho, sep, ao_progresso)?;
        let codificacao = opcoes.codificacao.unwrap_or(if bom {
            Codificacao::Utf8ComBom
        } else if varrido.utf8_valido {
            Codificacao::Utf8
        } else {
            Codificacao::Windows1252
        });

        let registros = registros_do_trecho(corpo, sep, truncada, REGISTROS_DA_AMOSTRA, codificacao);
        let decimal = decimal_pedido.unwrap_or_else(|| detectar_decimal(&registros, separador));
        let aspas = opcoes.aspas.unwrap_or_else(|| detectar_aspas(corpo, separador, truncada, codificacao, decimal));
        let cabecalho = opcoes.cabecalho.unwrap_or_else(|| detectar_cabecalho(&registros, decimal));
        let formato = Formato {
            codificacao,
            separador,
            fim_de_linha,
            aspas,
            cabecalho,
            decimal,
            termina_em_quebra: varrido.indice.termina_em_quebra,
        };
        let mut leitor = Leitor {
            caminho: caminho.to_path_buf(),
            arquivo: Some(arquivo),
            formato,
            indice: Arc::new(varrido.indice),
            modificado,
            nomes: Vec::new(),
            utf8_valido: varrido.utf8_valido,
        };
        leitor.calcular_nomes()?;
        Ok(leitor)
    }

    /// Um leitor sobre um arquivo cujo índice já se conhece (o que a gravação acabou de montar). Gravado em UTF-8, o
    /// texto é UTF-8 válido: a gravação só escreve texto do Rust e copia linhas de uma origem UTF-8 válida (com a
    /// origem inválida lida como UTF-8, ela recusa). Em Windows-1252 a pergunta não importa (fica `false`).
    pub(crate) fn com_indice(caminho: &Path, formato: Formato, indice: Indice) -> Resultado<Leitor> {
        let utf8_valido = formato.codificacao.e_utf8();
        let io = |e: std::io::Error| erro_io(&e, Some(caminho), CONTEXTO_LER);
        let arquivo = File::open(caminho).map_err(io)?;
        let meta = arquivo.metadata().map_err(io)?;
        if meta.len() != indice.tamanho {
            return Err(ErroBanco::com_status("O arquivo gravado não tem o tamanho esperado.", 500));
        }
        let mut leitor = Leitor {
            caminho: caminho.to_path_buf(),
            arquivo: Some(arquivo),
            formato,
            indice: Arc::new(indice),
            modificado: meta.modified().ok(),
            nomes: Vec::new(),
            utf8_valido,
        };
        leitor.calcular_nomes()?;
        Ok(leitor)
    }

    /// O cabeçalho (ou `coluna_N` onde faltar nome), um por coluna.
    fn calcular_nomes(&mut self) -> Resultado<()> {
        let n = self.colunas();
        let mut nomes = Vec::with_capacity(n);
        if self.formato.cabecalho && self.indice.registros > 0 {
            nomes = self.registros(0, 1)?.into_iter().next().unwrap_or_default();
        }
        while nomes.len() < n {
            nomes.push(format!("coluna_{}", nomes.len() + 1));
        }
        self.nomes = nomes;
        Ok(())
    }

    fn arquivo(&mut self) -> Resultado<&mut File> {
        if self.arquivo.is_none() {
            let f = File::open(&self.caminho).map_err(|e| erro_io(&e, Some(&self.caminho), CONTEXTO_LER))?;
            self.arquivo = Some(f);
        }
        self.arquivo.as_mut().ok_or_else(|| ErroBanco::com_status("O arquivo não está aberto.", 500))
    }

    /// Fecha o arquivo (ele reabre sozinho na próxima leitura): a gravação precisa do caminho livre.
    pub(crate) fn soltar(&mut self) {
        self.arquivo = None;
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    pub fn formato(&self) -> &Formato {
        &self.formato
    }

    pub fn indice(&self) -> &Indice {
        &self.indice
    }

    /// O tamanho do arquivo quando foi aberto, em bytes.
    pub fn tamanho(&self) -> u64 {
        self.indice.tamanho
    }

    /// Quando o arquivo foi modificado pela última vez, quando foi aberto.
    pub fn modificado(&self) -> Option<SystemTime> {
        self.modificado
    }

    /// Linhas de dados (sem o cabeçalho).
    pub fn linhas(&self) -> u64 {
        self.indice.registros.saturating_sub(u64::from(self.formato.cabecalho))
    }

    /// Colunas: a do registro mais largo (pelo menos uma).
    pub fn colunas(&self) -> usize {
        usize::try_from(self.indice.max_campos).unwrap_or(usize::MAX).max(1)
    }

    /// Os nomes das colunas: os do cabeçalho, e `coluna_N` onde ele não chega (ou sem cabeçalho).
    pub fn nomes(&self) -> &[String] {
        &self.nomes
    }

    /// Liga ou desliga "a primeira linha é o cabeçalho": só muda como o arquivo é lido.
    pub fn definir_cabecalho(&mut self, cabecalho: bool) -> Resultado<()> {
        self.formato.cabecalho = cabecalho;
        self.calcular_nomes()
    }

    /// O arquivo foi mudado por outro programa desde que foi aberto (tamanho ou data diferentes).
    pub fn mudou_no_disco(&self) -> bool {
        match std::fs::metadata(&self.caminho) {
            Ok(m) => m.len() != self.indice.tamanho || m.modified().ok() != self.modificado,
            Err(_) => true,
        }
    }

    /// Outro leitor sobre o mesmo arquivo, com o mesmo índice e um cursor próprio: para varrer o arquivo numa
    /// thread de fundo sem atrapalhar a leitura das páginas.
    pub fn reabrir_para_leitura(&self) -> Resultado<Leitor> {
        let arquivo = File::open(&self.caminho).map_err(|e| erro_io(&e, Some(&self.caminho), CONTEXTO_LER))?;
        Ok(Leitor {
            caminho: self.caminho.clone(),
            arquivo: Some(arquivo),
            formato: self.formato.clone(),
            indice: Arc::clone(&self.indice),
            modificado: self.modificado,
            nomes: self.nomes.clone(),
            utf8_valido: self.utf8_valido,
        })
    }

    /// O texto do arquivo todo é UTF-8 válido (visto na passada da abertura). Com a codificação UTF-8 escolhida
    /// para um arquivo que não é, as células leem U+FFFD no lugar dos bytes de outra codificação.
    pub fn utf8_valido(&self) -> bool {
        self.utf8_valido
    }

    fn conferir_arquivo(&mut self) -> Resultado<()> {
        let tamanho = self.indice.tamanho;
        let caminho = self.caminho.clone();
        let arquivo = self.arquivo()?;
        let atual = arquivo.metadata().map_err(|e| erro_io(&e, Some(&caminho), CONTEXTO_LER))?.len();
        if atual != tamanho {
            return Err(ErroBanco::com_status(
                "O arquivo foi alterado por outro programa depois de aberto. Abra-o de novo para ver o que há nele.",
                409,
            ));
        }
        Ok(())
    }

    /// Os registros `primeiro..primeiro + quantidade` do arquivo (o cabeçalho é o registro 0), cada um com os
    /// campos que tem (podem ser menos ou mais que [`Leitor::colunas`]).
    pub fn registros(&mut self, primeiro: u64, quantidade: usize) -> Resultado<Vec<Vec<String>>> {
        if quantidade == 0 {
            return Ok(Vec::new());
        }
        let Some((deslocamento, pular)) = self.indice.onde_comeca(primeiro) else {
            return Ok(Vec::new());
        };
        self.conferir_arquivo()?;
        let disponiveis = self.indice.registros - primeiro;
        let quantidade = usize::try_from(disponiveis.min(quantidade as u64)).unwrap_or(quantidade);
        let ultimo = primeiro + quantidade as u64 - 1;
        let fim = self.indice.onde_acaba(ultimo);
        let sep = self.formato.sep();
        let cod = self.formato.codificacao;
        let caminho = self.caminho.clone();
        let io = |e: std::io::Error| erro_io(&e, Some(&caminho), CONTEXTO_LER);
        let arquivo = self.arquivo()?;
        arquivo.seek(SeekFrom::Start(deslocamento)).map_err(io)?;
        let trecho = fim.saturating_sub(deslocamento);
        let capacidade = usize::try_from(trecho.min(BLOCO as u64)).unwrap_or(BLOCO);
        let mut v = Varredura::nova(arquivo.by_ref().take(trecho), deslocamento, sep, true, false, capacidade);
        v.pular(pular).map_err(io)?;
        let mut saida = Vec::with_capacity(quantidade);
        for _ in 0..quantidade {
            if !v.seguinte().map_err(io)? {
                break;
            }
            saida.push(v.todos_os_campos(cod));
        }
        Ok(saida)
    }

    /// As linhas de dados `primeira..primeira + quantidade` (sem o cabeçalho).
    pub fn linhas_de(&mut self, primeira: u64, quantidade: usize) -> Resultado<Vec<Vec<String>>> {
        self.registros(primeira + u64::from(self.formato.cabecalho), quantidade)
    }

    /// Uma linha de dados, se existe.
    pub fn linha(&mut self, linha: u64) -> Resultado<Option<Vec<String>>> {
        Ok(self.linhas_de(linha, 1)?.into_iter().next())
    }

    /// Percorre as linhas de dados do arquivo, da primeira à última, uma passada só. `quando_uma_linha(i, vista)`
    /// recebe o número da linha (de 0) e devolve se deve continuar; um erro dele sobe como veio. O
    /// `ao_progresso(bytes, total)` é chamado de tempos em tempos (e pode cancelar do mesmo jeito).
    pub fn varrer(
        &mut self,
        ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
        mut quando_uma_linha: impl FnMut(u64, &VistaLinha<'_>) -> Resultado<bool>,
    ) -> Resultado<()> {
        self.conferir_arquivo()?;
        let sep = self.formato.sep();
        let cod = self.formato.codificacao;
        let inicio = self.indice.inicio;
        let tamanho = self.indice.tamanho;
        let cabecalho = self.formato.cabecalho;
        let caminho = self.caminho.clone();
        let io = |e: std::io::Error| erro_io(&e, Some(&caminho), CONTEXTO_LER);
        let arquivo = self.arquivo()?;
        arquivo.seek(SeekFrom::Start(inicio)).map_err(io)?;
        let mut v = Varredura::nova(arquivo.by_ref(), inicio, sep, true, false, BLOCO);
        if cabecalho {
            v.pular(1).map_err(io)?;
        }
        let mut linha = 0u64;
        let mut proximo_aviso = inicio + (4 << 20);
        while v.seguinte().map_err(io)? {
            let vista = VistaLinha { buf: v.buffer(), campos: v.campos(), cod };
            if !quando_uma_linha(linha, &vista)? {
                return Ok(());
            }
            linha += 1;
            if v.deslocamento_do_fim() >= proximo_aviso {
                ao_progresso(v.deslocamento_do_fim(), tamanho)?;
                proximo_aviso = v.deslocamento_do_fim() + (4 << 20);
            }
        }
        ao_progresso(tamanho, tamanho)
    }

    /// As colunas pedidas (pelo número, de 0) de todas as linhas de dados, numa passada só: a origem do Trazer
    /// colunas lê a chave e as marcadas, e só elas. Cada coluna devolvida tem uma célula por linha (vazia se a
    /// linha é curta).
    pub fn colunas_de(
        &mut self,
        indices: &[usize],
        ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
    ) -> Resultado<Vec<Vec<String>>> {
        let linhas = usize::try_from(self.linhas()).unwrap_or(0);
        let mut saida: Vec<Vec<String>> = indices.iter().map(|_| Vec::with_capacity(linhas)).collect();
        self.varrer(ao_progresso, |_, vista| {
            for (coluna, &j) in saida.iter_mut().zip(indices) {
                coluna.push(vista.campo(j));
            }
            Ok(true)
        })?;
        Ok(saida)
    }

    /// Lê [`Leitor::colunas_de`] sem progresso.
    pub fn colunas_de_sem_progresso(&mut self, indices: &[usize]) -> Resultado<Vec<Vec<String>>> {
        self.colunas_de(indices, &mut sem_progresso)
    }
}
