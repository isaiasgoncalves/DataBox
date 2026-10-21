//! A busca da barra (Ctrl+F) na aba inteira: [`Planilha::buscar`] acha a próxima célula com o termo a partir de uma
//! posição, e [`Planilha::achar_todos`] entrega todas, na ordem das linhas e das colunas. Compara o texto como a
//! grade o mostra ([`Pasta::exibir`]: `1.234,5`, `15/03/2025`), sem diferenciar maiúsculas nem acentos
//! ([`comum::texto::Agulha`]). Lê o XML em fluxo ([`Pasta::linhas`]), sem índice e sem guardar as linhas.

use comum::Resultado;
use comum::texto::Agulha;

use crate::pasta::Pasta;
use crate::planilha::Planilha;

/// De quantas em quantas linhas a busca pergunta se deve parar.
const LINHAS_POR_CONSULTA: usize = 1024;

impl Planilha {
    /// A primeira célula com `texto` (sem diferenciar maiúsculas nem acentos) a partir de `a_partir_de` (linha do
    /// Excel, de 1, e coluna, de 0, inclusive), na ordem das linhas e, nelas, das colunas. Sem volta ao começo: `None`
    /// se não há nenhuma até o fim (ou se `parar` pediu). `texto` vazio não acha nada.
    pub fn buscar(
        &self,
        texto: &str,
        a_partir_de: (u32, u32),
        parar: &dyn Fn() -> bool,
    ) -> Resultado<Option<(u32, u32)>> {
        let Some(agulha) = Agulha::nova(texto) else { return Ok(None) };
        let (linha, coluna) = a_partir_de;
        let mut achada = None;
        percorrer(self.pasta(), self.indice_da_folha(), &agulha, linha, parar, &mut |l, c| {
            if l == linha && c < coluna {
                return true;
            }
            achada = Some((l, c));
            false
        })?;
        Ok(achada)
    }

    /// Todas as células com `texto`, na ordem: `ao_achar(linha, coluna)` (a linha do Excel, de 1; a coluna, de 0)
    /// recebe cada uma e devolve se continua. Devolve `true` se percorreu a aba até o fim.
    pub fn achar_todos(
        &self,
        texto: &str,
        parar: &dyn Fn() -> bool,
        ao_achar: &mut dyn FnMut(u32, u32) -> bool,
    ) -> Resultado<bool> {
        let Some(agulha) = Agulha::nova(texto) else { return Ok(true) };
        percorrer(self.pasta(), self.indice_da_folha(), &agulha, 1, parar, ao_achar)
    }
}

/// Percorre as linhas da aba `folha` a partir da linha `desde` e entrega cada célula com o termo. Devolve se chegou
/// ao fim.
fn percorrer(
    pasta: &Pasta,
    folha: usize,
    agulha: &Agulha,
    desde: u32,
    parar: &dyn Fn() -> bool,
    ao_achar: &mut dyn FnMut(u32, u32) -> bool,
) -> Resultado<bool> {
    for (i, linha) in pasta.linhas(folha)?.enumerate() {
        if i.is_multiple_of(LINHAS_POR_CONSULTA) && i > 0 && parar() {
            return Ok(false);
        }
        let linha = linha?;
        if linha.numero < desde {
            continue;
        }
        for celula in &linha.celulas {
            if !celula.valor.vazio() && agulha.em(&pasta.exibir(celula)) && !ao_achar(linha.numero, celula.coluna) {
                return Ok(false);
            }
        }
    }
    Ok(!parar())
}
