//! A leitura do arquivo (numa tarefa): o texto com a codificação e o fim de linha, e o que o corpo da tela precisa já
//! pronto para não pesar o quadro: o índice das linhas do leitor (arquivo acima de [`LIMITE_EDICAO`]) e a análise do
//! JSON ao abrir.

use std::path::Path;
use std::sync::Arc;

use ::texto::limites::LIMITE_EDICAO;
use ::texto::{Carregado, carregar};
use comum::Resultado;

use crate::TipoDeTexto;
use crate::json::Resposta;

/// O que a tarefa de leitura devolve.
pub(crate) struct Lido {
    /// A codificação, o fim de linha, o tamanho e a data (o texto fica em [`Lido::texto`]).
    pub carregado: Carregado,
    pub texto: Arc<String>,
    /// Acima de [`LIMITE_EDICAO`]: abre no leitor, só para leitura.
    pub grande: bool,
    /// O byte onde cada linha começa (só no leitor).
    pub linhas: Vec<u32>,
    /// O tamanho em bytes da maior linha (só no leitor: a largura da rolagem).
    pub maior_linha: usize,
    /// Quantos caracteres o texto tem (as Propriedades do leitor).
    pub caracteres: usize,
    /// A análise do JSON ao abrir (versão 0 do texto).
    pub json: Option<Resposta>,
}

/// Lê o arquivo e prepara o que o corpo precisa.
pub(crate) fn ler(caminho: &Path, tipo: TipoDeTexto) -> Resultado<Lido> {
    let mut carregado = carregar(caminho)?;
    let texto = Arc::new(std::mem::take(&mut carregado.texto));
    let grande = carregado.bytes > LIMITE_EDICAO || texto.len() as u64 > LIMITE_EDICAO;
    let (linhas, maior_linha) = if grande { indice_de_linhas(&texto) } else { (Vec::new(), 0) };
    let json = (tipo == TipoDeTexto::Json).then(|| Resposta::analisar(0, Arc::clone(&texto)));
    let caracteres = texto.chars().count();
    Ok(Lido { carregado, texto, grande, linhas, maior_linha, caracteres, json })
}

/// O byte onde cada linha começa e o tamanho (em bytes) da maior. Um `\n` fecha a linha; o que sobra depois do
/// último `\n` é mais uma linha, e texto que termina em `\n` não tem linha vazia no fim (como o rodapé conta).
pub(crate) fn indice_de_linhas(texto: &str) -> (Vec<u32>, usize) {
    let mut linhas = Vec::new();
    if texto.is_empty() {
        return (linhas, 0);
    }
    let mut maior = 0;
    let mut inicio = 0usize;
    linhas.push(0);
    for (i, _) in texto.match_indices('\n') {
        maior = maior.max(i - inicio);
        inicio = i + 1;
        if inicio < texto.len() {
            linhas.push(u32::try_from(inicio).unwrap_or(u32::MAX));
        }
    }
    if inicio < texto.len() {
        maior = maior.max(texto.len() - inicio);
    }
    (linhas, maior)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn indice_de_linhas_conta_como_o_rodape() {
        assert_eq!(indice_de_linhas(""), (vec![], 0));
        assert_eq!(indice_de_linhas("a\nbcd\n"), (vec![0, 2], 3));
        assert_eq!(indice_de_linhas("a\nbcd"), (vec![0, 2], 3));
        assert_eq!(indice_de_linhas("\n\n"), (vec![0, 1], 0));
        assert_eq!(indice_de_linhas("x"), (vec![0], 1));
        // o mesmo número que o `contar_linhas` do rodapé
        for t in ["", "a", "a\n", "a\nb", "a\nb\n", "\n", "\n\nx"] {
            assert_eq!(indice_de_linhas(t).0.len(), ::texto::posicao::contar_linhas(t), "{t:?}");
        }
    }
}
