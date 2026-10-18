//! As mensagens de erro, em português, prontas para a tela. Todas são erros de regra (400), com o detalhe
//! técnico (o texto do zip ou do XML) à parte; só "interrompido" tem código ([`Codigo::Cancelado`], 409).

use comum::{Codigo, ErroBanco};

use crate::xml::ErroXml;

const CONVERTER: &str = "Abra o arquivo no Excel e use Salvar Como › Pasta de Trabalho do Excel (.xlsx).";

pub(crate) fn vazio() -> ErroBanco {
    ErroBanco::novo("O arquivo está vazio; não é uma planilha do Excel (.xlsx).")
}

pub(crate) fn protegida() -> ErroBanco {
    ErroBanco::novo(
        "A planilha está protegida por senha e o DataBox não abre arquivos criptografados. No Excel, abra a planilha, \
         remova a senha (Arquivo › Informações › Proteger Pasta de Trabalho › Criptografar com Senha) e salve uma \
         cópia.",
    )
}

pub(crate) fn xls_antigo() -> ErroBanco {
    ErroBanco::novo(format!(
        "Este é o formato antigo do Excel (.xls, de 1997 a 2003), que o DataBox não lê. {CONVERTER}"
    ))
}

pub(crate) fn office_antigo() -> ErroBanco {
    ErroBanco::novo(
        "O arquivo não é uma planilha do Excel (.xlsx): é um documento do Office no formato antigo (de um programa \
         que não o Excel).",
    )
}

pub(crate) fn nao_e_pacote() -> ErroBanco {
    ErroBanco::novo(
        "O arquivo não é uma planilha do Excel (.xlsx): não tem a estrutura de um arquivo do Office. Se for CSV, \
         texto ou HTML, abra-o pelo editor de texto.",
    )
}

pub(crate) fn corrompido(detalhe: impl Into<String>) -> ErroBanco {
    ErroBanco::novo("O arquivo está corrompido ou incompleto e não pôde ser lido como planilha do Excel (.xlsx).")
        .detalhe(detalhe)
}

pub(crate) fn zip_de_outro_tipo(tipo: &str) -> ErroBanco {
    ErroBanco::novo(format!("O arquivo é {tipo}, não uma planilha do Excel (.xlsx)."))
}

pub(crate) fn zip_sem_excel() -> ErroBanco {
    ErroBanco::novo(
        "O arquivo é um pacote zip, mas não é uma pasta de trabalho do Excel (.xlsx): faltam as partes da planilha \
         (xl/workbook.xml).",
    )
}

pub(crate) fn xlsb() -> ErroBanco {
    ErroBanco::novo(format!("O formato binário do Excel (.xlsb) não é suportado. {CONVERTER}"))
}

pub(crate) fn parte_ausente(parte: &str) -> ErroBanco {
    ErroBanco::novo(format!("A planilha está corrompida: falta a parte «{parte}» do arquivo."))
}

pub(crate) fn folha_sem_parte(nome: &str) -> ErroBanco {
    ErroBanco::novo(format!("A planilha está corrompida: falta o conteúdo da aba «{nome}» no arquivo."))
}

pub(crate) fn sem_planilhas() -> ErroBanco {
    ErroBanco::novo("A pasta de trabalho não tem nenhuma planilha.")
}

pub(crate) fn folha_inexistente(indice: usize, total: usize) -> ErroBanco {
    ErroBanco::novo(format!("A aba {} não existe: a pasta de trabalho tem {total}.", indice + 1))
}

pub(crate) fn sem_celulas(nome: &str) -> ErroBanco {
    ErroBanco::novo(format!("A aba «{nome}» é um gráfico (ou outro tipo sem células), não uma planilha com dados."))
}

pub(crate) fn interrompido() -> ErroBanco {
    ErroBanco::com_status("Leitura da planilha interrompida.", 409).codigo(Codigo::Cancelado)
}

/// Falha ao ler uma parte: de XML malformado, de descompressão (CRC, fluxo inválido) ou de senha.
pub(crate) fn da_leitura(e: ErroXml, parte: &str) -> ErroBanco {
    match e {
        ErroXml::Malformado { posicao, motivo } => {
            ErroBanco::novo(format!("A planilha está corrompida: o conteúdo de «{parte}» não é um XML válido."))
                .detalhe(format!("{motivo} (byte {posicao})"))
        }
        ErroXml::Io(e) => da_io(&e, parte),
    }
}

pub(crate) fn da_io(e: &std::io::Error, parte: &str) -> ErroBanco {
    let texto = e.to_string();
    if texto.to_lowercase().contains("password") {
        return protegida();
    }
    corrompido(format!("não foi possível ler «{parte}»: {texto}"))
}

pub(crate) fn do_zip(e: &zip::result::ZipError, parte: &str) -> ErroBanco {
    match e {
        zip::result::ZipError::FileNotFound => parte_ausente(parte),
        zip::result::ZipError::Io(io) => da_io(io, parte),
        outro => {
            let texto = outro.to_string();
            if texto.to_lowercase().contains("password") {
                protegida()
            } else {
                corrompido(format!("«{parte}»: {texto}"))
            }
        }
    }
}
