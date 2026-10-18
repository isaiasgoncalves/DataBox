//! JSON em fluxo: uma lista de objetos {coluna: valor} (uma tabela ou consulta) ou, no banco inteiro,
//! {"banco", "exportado_em", "tabelas": {nome: [objetos]}}. Um objeto por vez, nunca a tabela inteira.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Write};

use comum::{Resultado, Valor};

use crate::arquivo::{CONTEXTO_GRAVAR, erro_io};
use crate::valores::{chave_livre, chaves_unicas, texto_json, valor_json};
use crate::{BLOCO_COPIA, Contexto, Meta};

/// Uma lista "[objeto, objeto, …]" sendo gravada; `nivel` é a profundidade dela no documento.
struct Lista {
    /// As chaves já em JSON ("\"id\""), sem repetição.
    chaves: Vec<Vec<u8>>,
    fora: String,
    quebra: String,
    indentado: bool,
    total: u64,
}

impl Lista {
    fn nova(colunas: &[String], indentado: bool, nivel: usize) -> Lista {
        let chaves = chaves_unicas(colunas)
            .iter()
            .map(|c| {
                let mut b = Vec::with_capacity(c.len() + 2);
                texto_json(&mut b, c);
                b
            })
            .collect();
        let fora = if indentado { "  ".repeat(nivel) } else { String::new() };
        let quebra = if indentado { format!("\n{fora}  ") } else { "\n".to_string() };
        Lista { chaves, fora, quebra, indentado, total: 0 }
    }

    /// O objeto como o `json.dumps(..., indent=2)` (ou compacto, com separadores "," e ":"), com as quebras
    /// de linha já recuadas para o nível da lista.
    fn objeto(&self, saida: &mut Vec<u8>, valores: &[Valor]) {
        if self.total > 0 {
            saida.push(b',');
        }
        saida.extend_from_slice(self.quebra.as_bytes());
        let pares = self.chaves.iter().zip(valores);
        if self.chaves.is_empty() || valores.is_empty() {
            saida.extend_from_slice(b"{}");
            return;
        }
        saida.push(b'{');
        for (i, (chave, valor)) in pares.enumerate() {
            if i > 0 {
                saida.push(b',');
            }
            if self.indentado {
                saida.extend_from_slice(self.quebra.as_bytes());
                saida.extend_from_slice(b"  ");
                saida.extend_from_slice(chave);
                saida.extend_from_slice(b": ");
            } else {
                saida.extend_from_slice(chave);
                saida.push(b':');
            }
            valor_json(saida, valor);
        }
        if self.indentado {
            saida.extend_from_slice(self.quebra.as_bytes());
        }
        saida.push(b'}');
    }

    fn fim(&self, saida: &mut Vec<u8>) {
        if self.total > 0 {
            saida.push(b'\n');
            saida.extend_from_slice(self.fora.as_bytes());
        }
        saida.push(b']');
    }
}

/// O arquivo JSON (de uma tabela ou do banco inteiro).
pub(crate) struct JsonArquivo {
    w: BufWriter<File>,
    varias: bool,
    indentado: bool,
    usados: HashSet<String>,
    tabelas: usize,
    lista: Option<Lista>,
    buf: Vec<u8>,
}

impl JsonArquivo {
    /// No banco inteiro, o começo do documento ({"banco", "exportado_em", "tabelas": {) já sai aqui.
    pub fn abrir(arquivo: File, varias: bool, indentado: bool, meta: Option<Meta>) -> Resultado<JsonArquivo> {
        let mut j = JsonArquivo {
            w: BufWriter::with_capacity(BLOCO_COPIA, arquivo),
            varias,
            indentado,
            usados: HashSet::new(),
            tabelas: 0,
            lista: None,
            buf: Vec::with_capacity(64 * 1024),
        };
        if varias {
            let meta = meta.unwrap_or_default();
            let exportado_em = meta
                .exportado_em
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string());
            let mut cabeca = Vec::new();
            let mut par = |chave: &str, valor: Option<&str>| {
                if indentado {
                    cabeca.extend_from_slice(b"  ");
                }
                texto_json(&mut cabeca, chave);
                cabeca.extend_from_slice(if indentado { b": " } else { b":" });
                match valor {
                    Some(v) => texto_json(&mut cabeca, v),
                    None => cabeca.extend_from_slice(b"null"),
                }
                cabeca.extend_from_slice(if indentado { b",\n" } else { b"," });
            };
            par("banco", meta.banco.as_deref());
            par("exportado_em", Some(&exportado_em));
            let mut texto = Vec::new();
            texto.extend_from_slice(if indentado { b"{\n" } else { b"{" });
            texto.extend_from_slice(&cabeca);
            texto.extend_from_slice(if indentado { b"  \"tabelas\": {" } else { b"\"tabelas\":{" });
            j.gravar(&texto)?;
        }
        Ok(j)
    }

    fn gravar(&mut self, dados: &[u8]) -> Resultado<()> {
        self.w.write_all(dados).map_err(|e| erro_io(&e, CONTEXTO_GRAVAR))
    }

    pub fn comecar(&mut self, nome: &str, colunas: &[String]) -> Resultado<()> {
        let mut texto = Vec::new();
        let nivel = if self.varias {
            let chave = chave_livre(nome, &mut self.usados);
            if self.tabelas > 0 {
                texto.push(b',');
            }
            if self.indentado {
                texto.extend_from_slice(b"\n    ");
                texto_json(&mut texto, &chave);
                texto.extend_from_slice(b": ");
            } else {
                texto.push(b'\n');
                texto_json(&mut texto, &chave);
                texto.push(b':');
            }
            2
        } else {
            0
        };
        texto.push(b'[');
        self.lista = Some(Lista::nova(colunas, self.indentado, nivel));
        self.gravar(&texto)
    }

    pub fn linha(&mut self, valores: &[Valor], ctx: &mut Contexto) -> Resultado<()> {
        let Some(lista) = self.lista.as_mut() else {
            return Err(crate::erro_sem_tabela());
        };
        self.buf.clear();
        lista.objeto(&mut self.buf, valores);
        lista.total += 1;
        self.w.write_all(&self.buf).map_err(|e| erro_io(&e, CONTEXTO_GRAVAR))?;
        ctx.progresso.mais_uma()
    }

    pub fn terminar(&mut self) -> Resultado<()> {
        let Some(lista) = self.lista.take() else {
            return Ok(());
        };
        let mut texto = Vec::new();
        lista.fim(&mut texto);
        self.tabelas += 1;
        self.gravar(&texto)
    }

    pub fn fechar(mut self) -> Resultado<()> {
        let fim: &[u8] = match (self.varias, self.indentado, self.tabelas > 0) {
            (false, _, _) => b"\n",
            (true, true, true) => b"\n  }\n}\n",
            (true, true, false) => b"}\n}\n",
            (true, false, true) => b"\n}}\n",
            (true, false, false) => b"}}\n",
        };
        self.gravar(fim)?;
        let arquivo = self.w.into_inner().map_err(|e| erro_io(e.error(), CONTEXTO_GRAVAR))?;
        drop(arquivo);
        Ok(())
    }

    pub fn abortar(self) {
        let (arquivo, _) = self.w.into_parts();
        drop(arquivo);
    }
}
