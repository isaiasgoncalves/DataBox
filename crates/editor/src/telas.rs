//! As telas dos outros crates: as de verdade com o recurso `telas` (padrão), os substitutos sem ele.

#[cfg(feature = "telas")]
pub(crate) use tela_consulta::{TelaConsulta, resumo_comando};
#[cfg(feature = "telas")]
pub(crate) use tela_dados::AbaDados;
#[cfg(feature = "telas")]
pub(crate) use tela_esquema::{AbaDefinicao, AbaEstrutura, AbaIndices, JanelaNovaTabela, JanelaNovoIndice};
#[cfg(feature = "telas")]
pub(crate) use tela_exportar::JanelaExportar;

#[cfg(not(feature = "telas"))]
pub(crate) use crate::substitutos::{
    AbaDados, AbaDefinicao, AbaEstrutura, AbaIndices, JanelaExportar, JanelaNovaTabela, JanelaNovoIndice, TelaConsulta,
    resumo_comando,
};

// a dica do Executar da Consulta SQL, para os testes conferirem a oferta
#[cfg(all(test, not(feature = "telas")))]
pub(crate) use crate::substitutos::DICA_EXECUTAR;
#[cfg(all(test, feature = "telas"))]
pub(crate) use tela_consulta::DICA_EXECUTAR;
