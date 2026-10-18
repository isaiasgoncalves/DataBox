//! As origens do "Trazer colunas" que o CSV oferece e usa: [`FonteCsv`] (um CSV aberto, com as
//! alterações ainda não salvas: o `Documento::fonte_de_colunas` do [`crate::TelaCsv`] e o "Outro arquivo") e
//! [`FonteTexto`] (o texto da área de transferência lido como tabela).
//!
//! `rotulo`, `nomes` e `linhas` não tocam a tabela (vêm de um resumo que a tela atualiza a cada retrato); só
//! [`FonteDeColunas::colunas`] lê, numa tarefa, com um instantâneo (o `Mutex` da tabela fica travado só um instante).

use std::sync::{Arc, Mutex};

use comum::{ErroBanco, Resultado};
use estado::{FonteDeColunas, TipoDeFonte};
use tabular::colunas::ColunaColada;

use crate::sessao::{self, TabelaCompartilhada};

/// O que a fonte sabe sem ler: os nomes das colunas e o número de linhas.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Resumo {
    pub nomes: Vec<String>,
    pub linhas: u64,
}

/// Um CSV como origem de colunas.
pub(crate) struct FonteCsv {
    pub rotulo: String,
    pub tabela: TabelaCompartilhada,
    pub resumo: Mutex<Resumo>,
}

impl FonteCsv {
    pub(crate) fn nova(rotulo: String, tabela: TabelaCompartilhada, resumo: Resumo) -> Arc<FonteCsv> {
        Arc::new(FonteCsv { rotulo, tabela, resumo: Mutex::new(resumo) })
    }

    pub(crate) fn atualizar(&self, resumo: Resumo) {
        *self.resumo.lock().unwrap_or_else(|e| e.into_inner()) = resumo;
    }
}

impl FonteDeColunas for FonteCsv {
    fn rotulo(&self) -> String {
        self.rotulo.clone()
    }

    fn tipo(&self) -> TipoDeFonte {
        TipoDeFonte::Csv
    }

    fn nomes(&self) -> Vec<String> {
        self.resumo.lock().unwrap_or_else(|e| e.into_inner()).nomes.clone()
    }

    fn linhas(&self) -> Option<u64> {
        Some(self.resumo.lock().unwrap_or_else(|e| e.into_inner()).linhas)
    }

    fn colunas(
        &self,
        quais: &[usize],
        ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
    ) -> Resultado<Vec<Vec<String>>> {
        sessao::ler_colunas(&self.tabela, quais, ao_progresso)
    }
}

/// O rótulo da área de transferência no seletor "De onde".
pub const ROTULO_AREA: &str = "Área de transferência";

/// O texto da área de transferência como origem (as colunas já separadas, com o cabeçalho quando há).
pub(crate) struct FonteTexto {
    pub colunas: Vec<ColunaColada>,
}

impl FonteDeColunas for FonteTexto {
    fn rotulo(&self) -> String {
        ROTULO_AREA.to_string()
    }

    fn tipo(&self) -> TipoDeFonte {
        TipoDeFonte::AreaDeTransferencia
    }

    fn nomes(&self) -> Vec<String> {
        self.colunas
            .iter()
            .enumerate()
            .map(|(i, c)| c.nome.clone().unwrap_or_else(|| format!("coluna_{}", i + 1)))
            .collect()
    }

    fn linhas(&self) -> Option<u64> {
        Some(self.colunas.iter().map(|c| c.valores.len()).max().unwrap_or(0) as u64)
    }

    fn colunas(
        &self,
        quais: &[usize],
        ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
    ) -> Resultado<Vec<Vec<String>>> {
        let n = usize::try_from(self.linhas().unwrap_or(0)).unwrap_or(0);
        let mut saida = Vec::with_capacity(quais.len());
        for (k, &c) in quais.iter().enumerate() {
            let coluna =
                self.colunas.get(c).ok_or_else(|| ErroBanco::novo(format!("A coluna {} não existe.", c + 1)))?;
            let mut valores = coluna.valores.clone();
            valores.resize(n, String::new());
            saida.push(valores);
            ao_progresso(k as u64 + 1, quais.len() as u64)?;
        }
        Ok(saida)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn fonte_de_texto_completa_as_colunas_curtas() {
        let f = FonteTexto {
            colunas: vec![
                ColunaColada { nome: Some("cod".into()), valores: vec!["1".into(), "2".into()] },
                ColunaColada { nome: None, valores: vec!["a".into()] },
            ],
        };
        assert_eq!(f.nomes(), ["cod", "coluna_2"]);
        assert_eq!(f.linhas(), Some(2));
        let c = f.colunas(&[1, 0], &mut |_, _| Ok(())).expect("ler");
        assert_eq!(c, [vec!["a", ""], vec!["1", "2"]]);
        assert!(f.colunas(&[5], &mut |_, _| Ok(())).is_err());
        assert_eq!(f.tipo(), TipoDeFonte::AreaDeTransferencia);
    }
}
