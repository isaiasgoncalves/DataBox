//! Os ícones do app por nome ([`Icone`]) e o [`pintar`]. Três famílias ([`Familia`]), como no `design/README.md`:
//!
//! - **silhuetas** (a barra de ferramentas, os botões de ícone, os botões com ícone e a aba Início): PNG brancos
//!   tingidos com a cor pedida (`--t2` no `.ib`, `--t3` nas listas, branco no `.ib.pri`); as partes de dois tons
//!   (`opacity:.6`) já vêm no alfa do PNG;
//! - **sólidos** (o trilho, o menu Arquivo, o banco, os objetos e as partes da tabela, as folhas de arquivo, as
//!   pastilhas de tipo e os estados): PNG com as cores deles, a cor pedida não vale. A folha
//!   ([`Icone::Folha`], a Definição e a Consulta) e a pastilha ([`Icone::Tipo`]) ganham o texto por cima, pintado
//!   pelo egui nas proporções do `.sk .rot` de `Main.dc.html` ([`retangulo_do_rotulo`], [`glifo_do_tipo`]);
//! - **traços** (×, +, as setas da árvore e dos menus e o ✓): continuam no pintor, com o traço de 1,6 da grade de
//!   24, nunca abaixo de 1px. O arco de carregamento do botão ocupado é o [`pintar_arco`].
//!
//! Os PNG saem de `design/icones` pelo rasterizador do repositório (`design/ferramentas/icones_app.py`) e ficam embutidos (`png.rs`, gerado por `icones_app.py --tabela`). O [`pintar`] escolhe
//! o PNG pelos **pixels** da tela (o menor que cobre o lado × `pixels_per_point`, ou o maior que houver): um ícone
//! de 16 pt a 125% usa o de 20, a 150% o de 24 e a 200% o de 32. Os sólidos não têm o 1,25×: a 125%, o
//! de 16 pt sai do PNG de 24 reduzido. Com o PNG a até 1px do tamanho pedido, ele é pintado no tamanho dele e preso
//! aos pixels (nítido); senão, reduzido com o mipmap. O desenho (o glifo "12" ou "123", o tamanho do rótulo) é
//! escolhido pelo lado **em pontos**. As texturas são carregadas uma vez, na primeira vez que cada uma
//! aparece, e ficam no `ctx.data`.
//!
//! ```ignore
//! let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
//! icones::pintar(ui.painter(), rect, Icone::Tabela, paleta(ui).estudio.t2);
//! icones::pintar(ui.painter(), rect, Icone::Tipo(TipoColuna::do_declarado("TEXT", false, false)), Color32::WHITE);
//! ```

// a tabela gerada: sem o rustfmt e com o tipo como o gerador escreve
#[rustfmt::skip]
#[allow(clippy::type_complexity)]
mod png;

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use egui::emath::GuiRounding;
use egui::epaint::{PathShape, PathStroke};
use egui::text::LayoutJob;
use egui::{Color32, ColorImage, Context, Id, Painter, Pos2, Rect, Stroke, TextureHandle, TextureId, Vec2, pos2, vec2};

use crate::fontes::{self, Peso};
use crate::tema::rgba;

/// A família do ícone: de onde vem o desenho e se a cor pedida vale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Familia {
    /// PNG branco, tingido com a cor pedida.
    Silhueta,
    /// PNG com as cores dele (a cor pedida não vale).
    Solido,
    /// Desenhado pelo pintor do egui, na cor pedida.
    Traco,
}

/// O tipo de uma coluna, para a pastilha (`eTipo`): a cor mora no PNG e o glifo ("12", "Aa", "PK"…) é pintado por
/// cima ([`glifo_do_tipo`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TipoColuna {
    Inteiro,
    Real,
    Texto,
    Blob,
    Data,
    Booleano,
    Json,
    /// A lista do JSON (`[]`): a mesma base do [`TipoColuna::Json`], com outro glifo; vale na estrutura do documento.
    Lista,
    ChavePrimaria,
    ChaveEstrangeira,
    /// Sem tipo declarado (o ∅).
    Nulo,
}

impl TipoColuna {
    pub const TODOS: [TipoColuna; 11] = [
        TipoColuna::Inteiro,
        TipoColuna::Real,
        TipoColuna::Texto,
        TipoColuna::Blob,
        TipoColuna::Data,
        TipoColuna::Booleano,
        TipoColuna::Json,
        TipoColuna::Lista,
        TipoColuna::ChavePrimaria,
        TipoColuna::ChaveEstrangeira,
        TipoColuna::Nulo,
    ];

    /// A pastilha de uma coluna pelo tipo declarado: a chave primária e a estrangeira primeiro; depois
    /// as exceções pelo nome do tipo (BOOLEAN/BOOL, DATE/DATETIME/TIMESTAMP/TIME, JSON); sem tipo, o ∅; o resto pela
    /// afinidade do SQLite (INTEGER → "12", REAL e NUMERIC → ",5", TEXT → "Aa", BLOB → "01").
    pub fn do_declarado(tipo: &str, pk: bool, fk: bool) -> TipoColuna {
        if pk {
            return TipoColuna::ChavePrimaria;
        }
        if fk {
            return TipoColuna::ChaveEstrangeira;
        }
        let t = tipo.trim();
        if t.is_empty() {
            return TipoColuna::Nulo;
        }
        // o nome sem o tamanho ("VARCHAR(20)" → "VARCHAR") nem as palavras seguintes ("UNSIGNED BIG INT")
        let nome = t.split(['(', ' ', '\t']).next().unwrap_or("").to_uppercase();
        match nome.as_str() {
            "BOOLEAN" | "BOOL" => return TipoColuna::Booleano,
            "DATE" | "DATETIME" | "TIMESTAMP" | "TIME" => return TipoColuna::Data,
            "JSON" => return TipoColuna::Json,
            _ => {}
        }
        match comum::afinidade(Some(t)) {
            comum::Afinidade::Integer => TipoColuna::Inteiro,
            comum::Afinidade::Real | comum::Afinidade::Numeric => TipoColuna::Real,
            comum::Afinidade::Text => TipoColuna::Texto,
            comum::Afinidade::Blob => TipoColuna::Blob,
        }
    }

    /// O nome para o leitor de tela.
    pub fn nome(self) -> &'static str {
        match self {
            TipoColuna::Inteiro => "Inteiro",
            TipoColuna::Real => "Real",
            TipoColuna::Texto => "Texto",
            TipoColuna::Blob => "BLOB",
            TipoColuna::Data => "Data",
            TipoColuna::Booleano => "Booleano",
            TipoColuna::Json => "JSON",
            TipoColuna::Lista => "Lista",
            TipoColuna::ChavePrimaria => "Chave primária",
            TipoColuna::ChaveEstrangeira => "Chave estrangeira",
            TipoColuna::Nulo => "Sem tipo",
        }
    }

    fn chave(self) -> &'static str {
        match self {
            TipoColuna::Inteiro => "tipos/inteiro",
            TipoColuna::Real => "tipos/real",
            TipoColuna::Texto => "tipos/texto",
            TipoColuna::Blob => "tipos/blob",
            TipoColuna::Data => "tipos/data",
            TipoColuna::Booleano => "tipos/booleano",
            // a lista usa o desenho do JSON: só o glifo muda ("[]" no lugar de "{}")
            TipoColuna::Json | TipoColuna::Lista => "tipos/json",
            TipoColuna::ChavePrimaria => "tipos/chave_primaria",
            TipoColuna::ChaveEstrangeira => "tipos/chave_estrangeira",
            TipoColuna::Nulo => "tipos/nulo",
        }
    }
}

/// O tipo de arquivo da folha (`eDoc`): a cor mora no PNG e o texto da etiqueta ("SQL", "XLSX"…) é pintado por cima.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TipoArquivo {
    Sql,
    Xlsx,
    Csv,
    Json,
    /// O texto simples (`.txt`, `.md`, `.log`), cinza `#6f7783`.
    Txt,
}

impl TipoArquivo {
    pub const TODOS: [TipoArquivo; 5] =
        [TipoArquivo::Sql, TipoArquivo::Xlsx, TipoArquivo::Csv, TipoArquivo::Json, TipoArquivo::Txt];

    /// O texto da etiqueta.
    pub fn rotulo(self) -> &'static str {
        match self {
            TipoArquivo::Sql => "SQL",
            TipoArquivo::Xlsx => "XLSX",
            TipoArquivo::Csv => "CSV",
            TipoArquivo::Json => "JSON",
            TipoArquivo::Txt => "TXT",
        }
    }

    fn chave(self) -> &'static str {
        match self {
            TipoArquivo::Sql => "folhas/sql",
            TipoArquivo::Xlsx => "folhas/xlsx",
            TipoArquivo::Csv => "folhas/csv",
            TipoArquivo::Json => "folhas/json",
            TipoArquivo::Txt => "folhas/txt",
        }
    }
}

/// Os sólidos de estado (`st_*` da biblioteca): os avisos, os toasts, o "i" da ajuda, a faixa de erro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EstadoIcone {
    /// O triângulo âmbar com o "!".
    Atencao,
    /// O círculo vermelho com o ×.
    Erro,
    /// O círculo verde com o ✓.
    Concluido,
    /// O círculo azul com o "i".
    Informacao,
    /// O círculo com o "?".
    Ajuda,
    /// O cadeado.
    Bloqueado,
}

impl EstadoIcone {
    pub const TODOS: [EstadoIcone; 6] = [
        EstadoIcone::Atencao,
        EstadoIcone::Erro,
        EstadoIcone::Concluido,
        EstadoIcone::Informacao,
        EstadoIcone::Ajuda,
        EstadoIcone::Bloqueado,
    ];

    pub fn nome(self) -> &'static str {
        match self {
            EstadoIcone::Atencao => "Atenção",
            EstadoIcone::Erro => "Erro",
            EstadoIcone::Concluido => "Concluído",
            EstadoIcone::Informacao => "Informação",
            EstadoIcone::Ajuda => "Ajuda",
            EstadoIcone::Bloqueado => "Bloqueado",
        }
    }

    fn chave(self) -> &'static str {
        match self {
            EstadoIcone::Atencao => "estados/atencao",
            EstadoIcone::Erro => "estados/erro",
            EstadoIcone::Concluido => "estados/concluido",
            EstadoIcone::Informacao => "estados/informacao",
            EstadoIcone::Ajuda => "estados/ajuda",
            EstadoIcone::Bloqueado => "estados/bloqueado",
        }
    }
}

/// Um ícone do app. O nome diz o que ele representa; [`Icone::nome`] é o texto que o leitor de tela ouve quando o
/// botão não diz outro. Quando uma silhueta e um sólido dizem a mesma coisa, o sólido leva o sufixo
/// (`PastaSolida`, `DocumentoSolido`, `NovoBancoSolido`, `DisqueteSolido`); a casa da aba Início é a silhueta
/// [`Icone::Casa`], e o [`Icone::Inicio`] é o sólido do trilho.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icone {
    // ---- silhuetas (barra de ferramentas, botões de ícone e com ícone)
    /// A seta de voltar (`Alt+←`; o ‹ da paginação).
    Voltar,
    /// A seta de avançar (`Alt+→`; o › da paginação).
    Avancar,
    /// A pasta de dois tons (Abrir pasta).
    Pasta,
    /// A folha com a dobra (Abrir arquivo).
    Arquivo,
    /// O disquete.
    Salvar,
    /// O quadrado com o "+" (Nova linha, Nova tabela).
    Novo,
    /// A coluna com o "+" ao lado (Nova coluna).
    NovaColuna,
    /// Duas folhas de dois tons.
    Duplicar,
    /// A lixeira.
    Excluir,
    /// A seta que volta em curva.
    Desfazer,
    /// A seta que avança em curva.
    Refazer,
    /// A lupa (a busca da barra e o filtro das listas).
    Buscar,
    /// O funil.
    Filtrar,
    /// A seta em círculo (`F5`).
    Atualizar,
    /// O triângulo de tocar (`Ctrl+Enter`).
    Executar,
    /// O quadrado de parar (Interromper consulta).
    Interromper,
    /// A janela de terminal com o `>_` (Nova consulta SQL).
    NovaConsulta,
    /// A seta para cima saindo da bandeja.
    Exportar,
    /// O retângulo com o painel à direita (Painel de propriedades).
    Painel,
    /// Os dois triângulos que se encontram (Recolher tudo).
    Recolher,
    /// A pasta com a seta (Mostrar na pasta).
    MostrarNaPasta,
    /// Copiar caminho.
    CopiarCaminho,
    /// O lápis (Renomear a tabela, Alterar a coluna).
    Editar,
    /// Subir a coluna.
    Subir,
    /// Descer a coluna.
    Descer,
    /// O ↵ (Inserir no editor).
    Inserir,
    /// O escudo (Verificar integridade).
    VerificarIntegridade,
    /// Os elos (Verificar chaves estrangeiras).
    VerificarChaves,
    /// A varinha (Otimizar, D7).
    Otimizar,
    /// As setas que convergem (Compactar (VACUUM)).
    Compactar,
    /// Os dois cilindros (Fazer cópia de segurança, D7).
    CopiaSeguranca,
    /// A casa da aba Início.
    Casa,
    /// O cilindro com o "+" (o botão Novo banco do Início).
    NovoBanco,
    /// As duas colunas com a seta entre elas (Trazer colunas).
    TrazerColunas,
    // ---- sólidos: o trilho, o menu Arquivo e as seções das Preferências
    /// A casa nova do trilho (Início).
    Inicio,
    /// A gaveta de fichário do trilho (Explorador).
    Explorador,
    /// O relógio com a seta do trilho (Histórico de consultas).
    Historico,
    /// As duas engrenagens (Preferências).
    Preferencias,
    /// A folha do menu Arquivo (Abrir arquivo).
    DocumentoSolido,
    /// O cilindro com o "+" do menu Arquivo (Novo banco).
    NovoBancoSolido,
    /// O disquete do menu Arquivo (Salvar).
    DisqueteSolido,
    /// A pasta (Abrir pasta no menu Arquivo; as pastas da etapa 4).
    PastaSolida,
    /// O relógio dos Abertos recentemente.
    Recentes,
    /// A seção Geral das Preferências.
    Geral,
    /// A seção Aparência das Preferências.
    Aparencia,
    /// A seção Exportação das Preferências.
    Exportacao,
    // ---- sólidos: o banco e os objetos
    /// O cilindro do SQLite, com as faixas e a pena.
    Banco,
    /// O cilindro cinza com o cadeado (banco aberto só para leitura).
    BancoSoLeitura,
    /// A ficha azul da tabela.
    Tabela,
    /// A ficha anil com a chave da tabela sem rowid (D6).
    TabelaSemRowid,
    /// A ficha da tabela virtual (FTS), com a lupa.
    TabelaVirtual,
    /// A ficha violeta da visão, com o olho.
    Visao,
    /// A ficha cinza dos objetos do sistema, com a engrenagem.
    Sistema,
    // ---- sólidos: as partes da tabela
    /// A grade com a célula escolhida.
    Dados,
    /// As colunas com as pastilhas de tipo.
    Estrutura,
    /// As barras ordenadas com o raio (Índices e gatilhos).
    Indices,
    /// As barras ordenadas (um índice).
    Indice,
    /// O raio (um gatilho).
    Gatilho,
    // ---- sólidos: os contadores da Visão geral
    /// O disco rígido aberto, de frente (Tamanho).
    Disco,
    /// A folha listrada com a linha do meio em azul (Linhas).
    Linhas,
    /// A tabela no desenho grande (`-grande`: 48, 72 e 96 px), da dock dos contadores da Visão geral.
    TabelaGrande,
    /// A visão no desenho grande, da dock.
    VisaoGrande,
    /// O índice no desenho grande, da dock.
    IndiceGrande,
    /// O gatilho no desenho grande, da dock.
    GatilhoGrande,
    /// O disco no desenho grande, da dock.
    DiscoGrande,
    /// As linhas no desenho grande, da dock.
    LinhasGrande,
    /// A folha SQL da Definição SQL (com a etiqueta "SQL").
    Definicao,
    /// A folha SQL das abas de Consulta (com a etiqueta "SQL").
    Consulta,
    // ---- sólidos com variante
    /// A pastilha do tipo de uma coluna, com o glifo por cima.
    Tipo(TipoColuna),
    /// A folha de um arquivo, com a etiqueta por cima.
    Folha(TipoArquivo),
    /// Um sólido de estado.
    Estado(EstadoIcone),
    // ---- traços (no pintor)
    /// O × (`M6 6l12 12M18 6 6 18`).
    Fechar,
    /// O + (`M12 5v14M5 12h14`).
    Mais,
    /// A seta da árvore fechada (`M9 6l6 6-6 6`), também a dos submenus.
    SetaDireita,
    /// A seta da árvore aberta (`M6 9l6 6 6-6`), também a das seções das Propriedades.
    SetaBaixo,
    /// O ✓ dos itens marcados dos menus (`M5 12.5l4.5 4.5L19 7.5`).
    Marcado,
    /// As duas janelas, a da frente à direita e embaixo (o "Destacar em janela" das docas, revisão de UI de
    /// 19/10/2026): o contorno da `.sil` de `Dados.dc.html` desenhado em traço.
    Destacar,
    /// O [`Icone::Destacar`] espelhado (o "Prender de volta" da janela própria de um painel).
    Prender,
}

impl Icone {
    /// Todos, na ordem do enum, com cada variante de [`Icone::Tipo`], [`Icone::Folha`] e [`Icone::Estado`] (o
    /// mostruário e os testes).
    pub const TODOS: [Icone; 97] = [
        Icone::Voltar,
        Icone::Avancar,
        Icone::Pasta,
        Icone::Arquivo,
        Icone::Salvar,
        Icone::Novo,
        Icone::NovaColuna,
        Icone::Duplicar,
        Icone::Excluir,
        Icone::Desfazer,
        Icone::Refazer,
        Icone::Buscar,
        Icone::Filtrar,
        Icone::Atualizar,
        Icone::Executar,
        Icone::Interromper,
        Icone::NovaConsulta,
        Icone::Exportar,
        Icone::Painel,
        Icone::Recolher,
        Icone::MostrarNaPasta,
        Icone::CopiarCaminho,
        Icone::Editar,
        Icone::Subir,
        Icone::Descer,
        Icone::Inserir,
        Icone::VerificarIntegridade,
        Icone::VerificarChaves,
        Icone::Otimizar,
        Icone::Compactar,
        Icone::CopiaSeguranca,
        Icone::Casa,
        Icone::NovoBanco,
        Icone::TrazerColunas,
        Icone::Inicio,
        Icone::Explorador,
        Icone::Historico,
        Icone::Preferencias,
        Icone::DocumentoSolido,
        Icone::NovoBancoSolido,
        Icone::DisqueteSolido,
        Icone::PastaSolida,
        Icone::Recentes,
        Icone::Geral,
        Icone::Aparencia,
        Icone::Exportacao,
        Icone::Banco,
        Icone::BancoSoLeitura,
        Icone::Tabela,
        Icone::TabelaSemRowid,
        Icone::TabelaVirtual,
        Icone::Visao,
        Icone::Sistema,
        Icone::Dados,
        Icone::Estrutura,
        Icone::Indices,
        Icone::Indice,
        Icone::Gatilho,
        Icone::Disco,
        Icone::Linhas,
        Icone::TabelaGrande,
        Icone::VisaoGrande,
        Icone::IndiceGrande,
        Icone::GatilhoGrande,
        Icone::DiscoGrande,
        Icone::LinhasGrande,
        Icone::Definicao,
        Icone::Consulta,
        Icone::Tipo(TipoColuna::Inteiro),
        Icone::Tipo(TipoColuna::Real),
        Icone::Tipo(TipoColuna::Texto),
        Icone::Tipo(TipoColuna::Blob),
        Icone::Tipo(TipoColuna::Data),
        Icone::Tipo(TipoColuna::Booleano),
        Icone::Tipo(TipoColuna::Json),
        Icone::Tipo(TipoColuna::Lista),
        Icone::Tipo(TipoColuna::ChavePrimaria),
        Icone::Tipo(TipoColuna::ChaveEstrangeira),
        Icone::Tipo(TipoColuna::Nulo),
        Icone::Folha(TipoArquivo::Sql),
        Icone::Folha(TipoArquivo::Xlsx),
        Icone::Folha(TipoArquivo::Csv),
        Icone::Folha(TipoArquivo::Json),
        Icone::Folha(TipoArquivo::Txt),
        Icone::Estado(EstadoIcone::Atencao),
        Icone::Estado(EstadoIcone::Erro),
        Icone::Estado(EstadoIcone::Concluido),
        Icone::Estado(EstadoIcone::Informacao),
        Icone::Estado(EstadoIcone::Ajuda),
        Icone::Estado(EstadoIcone::Bloqueado),
        Icone::Fechar,
        Icone::Mais,
        Icone::SetaDireita,
        Icone::SetaBaixo,
        Icone::Marcado,
        Icone::Destacar,
        Icone::Prender,
    ];

    /// O nome do ícone para o leitor de tela (o `aria-label` das pranchetas): o rótulo padrão do
    /// [`crate::pecas::botao::Botao::icone`].
    pub fn nome(self) -> &'static str {
        match self {
            Icone::Voltar => "Voltar",
            Icone::Avancar => "Avançar",
            Icone::Pasta => "Abrir pasta",
            Icone::Arquivo => "Abrir arquivo",
            Icone::Salvar => "Salvar",
            Icone::Novo => "Novo",
            Icone::NovaColuna => "Nova coluna",
            Icone::Duplicar => "Duplicar",
            Icone::Excluir => "Excluir",
            Icone::Desfazer => "Desfazer",
            Icone::Refazer => "Refazer",
            Icone::Buscar => "Buscar",
            Icone::Filtrar => "Filtrar",
            Icone::Atualizar => "Atualizar",
            Icone::Executar => "Executar",
            Icone::Interromper => "Interromper",
            Icone::NovaConsulta => "Nova consulta",
            Icone::Exportar => "Exportar",
            Icone::Painel => "Painel de propriedades",
            Icone::Recolher => "Recolher tudo",
            Icone::MostrarNaPasta => "Mostrar na pasta",
            Icone::CopiarCaminho => "Copiar caminho",
            Icone::Editar => "Editar",
            Icone::Subir => "Subir",
            Icone::Descer => "Descer",
            Icone::Inserir => "Inserir no editor",
            Icone::VerificarIntegridade => "Verificar integridade",
            Icone::VerificarChaves => "Verificar chaves estrangeiras",
            Icone::Otimizar => "Otimizar",
            Icone::Compactar => "Compactar (VACUUM)",
            Icone::CopiaSeguranca => "Fazer cópia de segurança",
            Icone::Casa | Icone::Inicio => "Início",
            Icone::NovoBanco | Icone::NovoBancoSolido => "Novo banco",
            Icone::TrazerColunas => "Trazer colunas",
            Icone::Explorador => "Explorador",
            Icone::Historico => "Histórico de consultas",
            Icone::Preferencias => "Preferências",
            Icone::DocumentoSolido => "Arquivo",
            Icone::DisqueteSolido => "Salvar",
            Icone::PastaSolida => "Pasta",
            Icone::Recentes => "Abertos recentemente",
            Icone::Geral => "Geral",
            Icone::Aparencia => "Aparência",
            Icone::Exportacao => "Exportação",
            Icone::Banco => "Banco",
            Icone::BancoSoLeitura => "Banco só leitura",
            Icone::Tabela | Icone::TabelaGrande => "Tabela",
            Icone::TabelaSemRowid => "Tabela sem rowid",
            Icone::TabelaVirtual => "Tabela virtual",
            Icone::Visao | Icone::VisaoGrande => "Visão",
            Icone::Sistema => "Objeto do sistema",
            Icone::Dados => "Dados",
            Icone::Estrutura => "Estrutura",
            Icone::Indices => "Índices e gatilhos",
            Icone::Indice | Icone::IndiceGrande => "Índice",
            Icone::Gatilho | Icone::GatilhoGrande => "Gatilho",
            Icone::Disco | Icone::DiscoGrande => "Tamanho",
            Icone::Linhas | Icone::LinhasGrande => "Linhas",
            Icone::Definicao => "Definição SQL",
            Icone::Consulta => "Consulta",
            Icone::Tipo(t) => t.nome(),
            Icone::Folha(a) => a.rotulo(),
            Icone::Estado(e) => e.nome(),
            Icone::Fechar => "Fechar",
            Icone::Mais => "Mais",
            Icone::SetaDireita => "Expandir",
            Icone::SetaBaixo => "Recolher",
            Icone::Marcado => "Marcado",
            Icone::Destacar => "Destacar em janela",
            Icone::Prender => "Prender de volta",
        }
    }

    /// A família: silhueta (tingida), sólido (cores próprias) ou traço (no pintor).
    pub fn familia(self) -> Familia {
        match self {
            Icone::Fechar
            | Icone::Mais
            | Icone::SetaDireita
            | Icone::SetaBaixo
            | Icone::Marcado
            | Icone::Destacar
            | Icone::Prender => Familia::Traco,
            Icone::Voltar
            | Icone::Avancar
            | Icone::Pasta
            | Icone::Arquivo
            | Icone::Salvar
            | Icone::Novo
            | Icone::NovaColuna
            | Icone::Duplicar
            | Icone::Excluir
            | Icone::Desfazer
            | Icone::Refazer
            | Icone::Buscar
            | Icone::Filtrar
            | Icone::Atualizar
            | Icone::Executar
            | Icone::Interromper
            | Icone::NovaConsulta
            | Icone::Exportar
            | Icone::Painel
            | Icone::Recolher
            | Icone::MostrarNaPasta
            | Icone::CopiarCaminho
            | Icone::Editar
            | Icone::Subir
            | Icone::Descer
            | Icone::Inserir
            | Icone::VerificarIntegridade
            | Icone::VerificarChaves
            | Icone::Otimizar
            | Icone::Compactar
            | Icone::CopiaSeguranca
            | Icone::Casa
            | Icone::NovoBanco
            | Icone::TrazerColunas => Familia::Silhueta,
            _ => Familia::Solido,
        }
    }

    /// A chave na tabela dos PNG (`pasta/nome`, a pasta e o nome do arquivo em `assets/icones`); `None` nos traços.
    pub fn chave_png(self) -> Option<&'static str> {
        let chave = match self {
            Icone::Voltar => "silhuetas/voltar",
            Icone::Avancar => "silhuetas/avancar",
            Icone::Pasta => "silhuetas/pasta",
            Icone::Arquivo => "silhuetas/arquivo",
            Icone::Salvar => "silhuetas/salvar",
            Icone::Novo => "silhuetas/novo",
            Icone::NovaColuna => "silhuetas/nova_coluna",
            Icone::Duplicar => "silhuetas/duplicar",
            Icone::Excluir => "silhuetas/excluir",
            Icone::Desfazer => "silhuetas/desfazer",
            Icone::Refazer => "silhuetas/refazer",
            Icone::Buscar => "silhuetas/buscar",
            Icone::Filtrar => "silhuetas/filtrar",
            Icone::Atualizar => "silhuetas/atualizar",
            Icone::Executar => "silhuetas/executar",
            Icone::Interromper => "silhuetas/interromper",
            Icone::NovaConsulta => "silhuetas/nova_consulta",
            Icone::Exportar => "silhuetas/exportar",
            Icone::Painel => "silhuetas/painel",
            Icone::Recolher => "silhuetas/recolher",
            Icone::MostrarNaPasta => "silhuetas/mostrar_na_pasta",
            Icone::CopiarCaminho => "silhuetas/copiar_caminho",
            Icone::Editar => "silhuetas/editar",
            Icone::Subir => "silhuetas/subir",
            Icone::Descer => "silhuetas/descer",
            Icone::Inserir => "silhuetas/inserir",
            Icone::VerificarIntegridade => "silhuetas/verificar_integridade",
            Icone::VerificarChaves => "silhuetas/verificar_chaves",
            Icone::Otimizar => "silhuetas/otimizar",
            Icone::Compactar => "silhuetas/compactar",
            Icone::CopiaSeguranca => "silhuetas/copia_seguranca",
            Icone::Casa => "silhuetas/inicio",
            Icone::NovoBanco => "silhuetas/novo_banco",
            Icone::TrazerColunas => "silhuetas/trazer_colunas",
            Icone::Inicio => "solidos/inicio",
            Icone::Explorador => "solidos/explorador",
            Icone::Historico => "solidos/historico",
            Icone::Preferencias => "solidos/preferencias",
            Icone::DocumentoSolido => "solidos/documento",
            Icone::NovoBancoSolido => "solidos/novo_banco",
            Icone::DisqueteSolido => "solidos/salvar",
            Icone::PastaSolida => "solidos/pasta",
            Icone::Recentes => "solidos/recentes",
            Icone::Geral => "solidos/geral",
            Icone::Aparencia => "solidos/aparencia",
            Icone::Exportacao => "solidos/exportacao",
            Icone::Banco => "solidos/banco",
            Icone::BancoSoLeitura => "solidos/banco_so_leitura",
            Icone::Tabela => "solidos/tabela",
            Icone::TabelaSemRowid => "solidos/tabela_sem_rowid",
            Icone::TabelaVirtual => "solidos/tabela_virtual",
            Icone::Visao => "solidos/visao",
            Icone::Sistema => "solidos/sistema",
            Icone::Dados => "solidos/dados",
            Icone::Estrutura => "solidos/estrutura",
            Icone::Indices => "solidos/indices",
            Icone::Indice => "solidos/indice",
            Icone::Gatilho => "solidos/gatilho",
            Icone::Disco => "solidos/disco",
            Icone::Linhas => "solidos/linhas",
            Icone::TabelaGrande => "solidos/tabela-g",
            Icone::VisaoGrande => "solidos/visao-g",
            Icone::IndiceGrande => "solidos/indice-g",
            Icone::GatilhoGrande => "solidos/gatilho-g",
            Icone::DiscoGrande => "solidos/disco-g",
            Icone::LinhasGrande => "solidos/linhas-g",
            Icone::Definicao | Icone::Consulta => TipoArquivo::Sql.chave(),
            Icone::Tipo(t) => t.chave(),
            Icone::Folha(a) => a.chave(),
            Icone::Estado(e) => e.chave(),
            Icone::Fechar
            | Icone::Mais
            | Icone::SetaDireita
            | Icone::SetaBaixo
            | Icone::Marcado
            | Icone::Destacar
            | Icone::Prender => return None,
        };
        Some(chave)
    }

    /// Os PNG do ícone, `(lado em pixels, bytes)` do menor para o maior (vazio nos traços). A tabela gerada fica
    /// indexada pela chave (montada uma vez): o `pintar` chama isto para cada ícone a cada quadro.
    pub fn pngs(self) -> &'static [(u32, &'static [u8])] {
        type Lista = &'static [(u32, &'static [u8])];
        static POR_CHAVE: LazyLock<HashMap<&'static str, Lista>> = LazyLock::new(|| png::PNG.iter().copied().collect());
        let Some(chave) = self.chave_png() else { return &[] };
        POR_CHAVE.get(chave).copied().unwrap_or_default()
    }

    /// O que vai pintado por cima do PNG: o glifo da pastilha ou o texto da etiqueta da folha.
    fn rotulo(self) -> Option<Rotulo> {
        match self {
            Icone::Tipo(t) => Some(Rotulo::Tipo(t)),
            Icone::Folha(a) => Some(Rotulo::Folha(a.rotulo())),
            Icone::Definicao | Icone::Consulta => Some(Rotulo::Folha(TipoArquivo::Sql.rotulo())),
            _ => None,
        }
    }
}

/// Todas as linhas da tabela dos PNG, `(pasta/nome, [(lado, bytes)])` (os testes conferem a pasta `assets/icones`
/// contra ela).
pub fn chaves_png() -> impl Iterator<Item = (&'static str, &'static [(u32, &'static [u8])])> {
    png::PNG.iter().copied()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rotulo {
    Tipo(TipoColuna),
    Folha(&'static str),
}

// ------------------------------------------------------------------ escolher o PNG

/// O PNG para um lado em pixels físicos: o índice do menor que cobre `pixels − 0,5` (o padrão do `marca.rs`), ou o
/// do maior que houver. `tamanhos` vai do menor para o maior.
pub fn escolher_png(tamanhos: &[u32], pixels: f32) -> Option<usize> {
    indice_do_png(tamanhos.iter().copied(), pixels)
}

/// A regra do [`escolher_png`] sobre os lados, sem montar uma lista.
fn indice_do_png(mut lados: impl ExactSizeIterator<Item = u32>, pixels: f32) -> Option<usize> {
    let n = lados.len();
    if n == 0 {
        return None;
    }
    Some(lados.position(|l| l as f32 >= pixels - 0.5).unwrap_or(n - 1))
}

/// O PNG (`(lado, bytes)`) de um ícone para um lado em pixels físicos ([`escolher_png`]).
fn png_para(pngs: &'static [(u32, &'static [u8])], pixels: f32) -> Option<(u32, &'static [u8])> {
    indice_do_png(pngs.iter().map(|(l, _)| *l), pixels).and_then(|i| pngs.get(i).copied())
}

/// O lado do PNG que um ícone de `lado_pontos` usa a `pixels_por_ponto` (os testes e quem precisa medir).
pub fn lado_do_png(icone: Icone, lado_pontos: f32, pixels_por_ponto: f32) -> Option<u32> {
    png_para(icone.pngs(), lado_pontos * pixels_por_ponto).map(|(lado, _)| lado)
}

/// Onde o PNG de `lado_png` pixels vai na tela para um ícone de `lado` pontos centrado em `centro`: no tamanho do
/// PNG quando ele fica a até 1px do pedido (pixel a pixel, nítido), senão no tamanho pedido arredondado aos pixels;
/// o canto preso aos pixels físicos.
pub fn retangulo_na_tela(centro: Pos2, lado: f32, lado_png: u32, pixels_por_ponto: f32) -> Rect {
    let pedido = lado * pixels_por_ponto;
    let lado_px = if (lado_png as f32 - pedido).abs() <= 1.0 { lado_png as f32 } else { pedido.round().max(1.0) };
    let lado_tela = lado_px / pixels_por_ponto;
    let canto = (centro - Vec2::splat(lado_tela / 2.0)).round_to_pixels(pixels_por_ponto);
    Rect::from_min_size(canto, Vec2::splat(lado_tela))
}

// ------------------------------------------------------------------ as texturas

/// (chave, lado do PNG, cinza em %) → textura.
type Cache = HashMap<(&'static str, u32, u8), TextureHandle>;

fn id_cache() -> Id {
    Id::new("databox::icones")
}

/// O `filter: grayscale(a)` do CSS sobre um pixel RGBA não pré-multiplicado.
fn cinza_css(px: &mut [u8; 4], a: f32) {
    let s = 1.0 - a.clamp(0.0, 1.0);
    let (r, g, b) = (f32::from(px[0]), f32::from(px[1]), f32::from(px[2]));
    let canal = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    px[0] = canal((0.2126 + 0.7874 * s) * r + (0.7152 - 0.7152 * s) * g + (0.0722 - 0.0722 * s) * b);
    px[1] = canal((0.2126 - 0.2126 * s) * r + (0.7152 + 0.2848 * s) * g + (0.0722 - 0.0722 * s) * b);
    px[2] = canal((0.2126 - 0.2126 * s) * r + (0.7152 - 0.7152 * s) * g + (0.0722 + 0.9278 * s) * b);
}

fn carregar(ctx: &Context, chave: &str, lado: u32, bytes: &[u8], cinza: u8) -> Option<TextureHandle> {
    let (mut rgba, w, h) = crate::marca::decodificar_png(bytes)?;
    if cinza > 0 {
        for px in rgba.as_chunks_mut::<4>().0 {
            cinza_css(px, f32::from(cinza) / 100.0);
        }
    }
    let img = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    let opcoes =
        egui::TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..egui::TextureOptions::LINEAR };
    Some(ctx.load_texture(format!("databox-icone-{chave}-{lado}-{cinza}"), img, opcoes))
}

/// A textura (carregada na primeira vez; o carregamento fica fora do `data_mut`, que travaria o contexto). A busca só
/// copia o `Arc`; a inserção mexe no mapa guardado, sem copiar o mapa (nenhuma outra cópia do `Arc` vive então).
fn textura(ctx: &Context, chave: &'static str, lado: u32, bytes: &[u8], cinza: u8) -> Option<TextureId> {
    let k = (chave, lado, cinza);
    let achada = ctx.data(|d| d.get_temp::<Arc<Cache>>(id_cache()).and_then(|c| c.get(&k).map(TextureHandle::id)));
    if achada.is_some() {
        return achada;
    }
    let t = carregar(ctx, chave, lado, bytes, cinza)?;
    let id = t.id();
    ctx.data_mut(|d| Arc::make_mut(d.get_temp_mut_or_default::<Arc<Cache>>(id_cache())).insert(k, t));
    Some(id)
}

/// A textura que o ícone de `lado_pontos` usa na escala de agora, se já foi carregada (para os testes acharem o
/// ícone entre as formas pintadas).
pub fn textura_carregada(ctx: &Context, icone: Icone, lado_pontos: f32) -> Option<TextureId> {
    let chave = icone.chave_png()?;
    let lado = lado_do_png(icone, lado_pontos, ctx.pixels_per_point())?;
    let cache: Arc<Cache> = ctx.data(|d| d.get_temp(id_cache()))?;
    cache.get(&(chave, lado, 0)).map(TextureHandle::id)
}

/// A textura do PNG que o ícone de `lado_pontos` usa na escala de agora (carregada na primeira vez), com o lado do
/// PNG em pixels: para quem pinta o PNG de outro jeito que o [`pintar`] (o reflexo da dock, `pecas::dock`). `None`
/// nos traços.
pub fn textura_do_icone(ctx: &Context, icone: Icone, lado_pontos: f32) -> Option<(TextureId, u32)> {
    let chave = icone.chave_png()?;
    let (lado, bytes) = png_para(icone.pngs(), lado_pontos * ctx.pixels_per_point())?;
    textura(ctx, chave, lado, bytes, 0).map(|t| (t, lado))
}

// ------------------------------------------------------------------ pintar

/// Pinta o ícone no quadrado centrado em `rect` (o lado é o menor dos dois): 16 na barra de ferramentas, 14 no
/// `.ib.pq`, nos botões com ícone e nas silhuetas da árvore, 12 no cabeçalho dos painéis, 22 no trilho, 24 no menu
/// Arquivo. As silhuetas e os traços usam `cor`; os sólidos, as cores deles. A opacidade do `painter` vale para
/// todos (o botão desligado). Não aloca espaço.
pub fn pintar(painter: &Painter, rect: Rect, icone: Icone, cor: Color32) {
    pintar_com_cinza(painter, rect, icone, cor, 0.0);
}

/// O [`pintar`] com o `filter: grayscale(cinza)` do CSS (de 0 a 1): o sólido do item desligado do menu (`.mi.off
/// .mi-ic`, 0,7) e o `.sk.inativo` (1). A opacidade fica com o `painter`.
pub fn pintar_apagado(painter: &Painter, rect: Rect, icone: Icone, cinza: f32) {
    pintar_com_cinza(painter, rect, icone, Color32::WHITE, cinza);
}

fn pintar_com_cinza(painter: &Painter, rect: Rect, icone: Icone, cor: Color32, cinza: f32) {
    if !rect.is_finite() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let lado = rect.width().min(rect.height());
    let Some(chave) = icone.chave_png() else {
        pintar_traco(painter, Rect::from_center_size(rect.center(), Vec2::splat(lado)), icone, cor);
        return;
    };
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let Some((lado_png, bytes)) = png_para(icone.pngs(), lado * ppp) else { return };
    let cinza = (cinza.clamp(0.0, 1.0) * 100.0).round() as u8;
    let Some(id) = textura(ctx, chave, lado_png, bytes, cinza) else { return };
    let destino = retangulo_na_tela(rect.center(), lado, lado_png, ppp);
    let tinta = if icone.familia() == Familia::Silhueta { cor } else { Color32::WHITE };
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    painter.image(id, destino, uv, tinta);
    if let Some(r) = icone.rotulo() {
        pintar_rotulo(painter, destino, lado, r);
    }
}

// ------------------------------------------------------------------ o texto por cima

/// O desenho da etiqueta da folha: a grande (`eDoc`, até 24 pt) ou a menor (`eDocG`, 32 pt para cima). Na etapa 3 só
/// a grande é rasterizada (também em 32 px, para os 16 pt a 200%); a menor fica pronta para a etapa 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etiqueta {
    Grande,
    Menor,
}

/// O retângulo do rótulo da folha num quadrado de `lado` com o canto em (0, 0), pelo `.sk.arq .rot`: 4,2% / 43,75% /
/// 83,3% / 43,75% na etiqueta grande; 6,25% / 47,9% / 75% / 33,3% na menor.
pub fn retangulo_do_rotulo(lado: f32, etiqueta: Etiqueta) -> Rect {
    let (x, y, w, h) = match etiqueta {
        Etiqueta::Grande => (0.042, 0.4375, 0.833, 0.4375),
        Etiqueta::Menor => (0.0625, 0.479, 0.75, 0.333),
    };
    Rect::from_min_size(pos2(x * lado, y * lado), vec2(w * lado, h * lado))
}

/// O glifo da pastilha pelo lado **em pontos**: até 24 pt, "12", ",5", "Aa", "01", "31", "✓", "{}", "PK", "FK" e
/// "∅"; acima de 24 pt, o inteiro e o real ficam "123" e "1,5". A prancheta 12 usa "12" e ",5" também nas pastilhas de
/// 24 da ficha da linha (a biblioteca, prancheta 9, só tem "123" e "1,5" no desenho de 48); decidido na revisão de
/// 17/10/2026.
pub fn glifo_do_tipo(tipo: TipoColuna, lado_pontos: f32) -> &'static str {
    let grande = lado_pontos > 24.0;
    match tipo {
        TipoColuna::Inteiro if grande => "123",
        TipoColuna::Inteiro => "12",
        TipoColuna::Real if grande => "1,5",
        TipoColuna::Real => ",5",
        TipoColuna::Texto => "Aa",
        TipoColuna::Blob => "01",
        TipoColuna::Data => "31",
        TipoColuna::Booleano => "✓",
        TipoColuna::Json => "{}",
        TipoColuna::Lista => "[]",
        TipoColuna::ChavePrimaria => "PK",
        TipoColuna::ChaveEstrangeira => "FK",
        TipoColuna::Nulo => "∅",
    }
}

/// Tamanho da fonte e espaçamento entre letras do glifo da pastilha de `lado` pontos (`.sk.tipo.g1/g2/g3`: 10, 7,5 e
/// 6,5 px a 16 pt; 15, 11 e 9,5 a 24 pt; nos outros lados, o de 16 na proporção). O espaçamento é o `.02em`.
pub fn fonte_do_glifo(glifo: &str, lado: f32) -> (f32, f32) {
    let n = glifo.chars().count().clamp(1, 3);
    let tamanho =
        if (lado - 24.0).abs() < 0.01 { [15.0, 11.0, 9.5][n - 1] } else { [10.0, 7.5, 6.5][n - 1] * lado / 16.0 };
    (tamanho, 0.02 * tamanho)
}

/// Tamanho da fonte, espaçamento entre letras e a sombra `(deslocamento, alfa)` do texto da folha de `lado` pontos
/// (`.sk.s16.arq .rot`: 5,5 px, −0,15 e `0 .5px .5px rgba(0,0,0,.45)`; com 4 letras ou mais, o `l4`, 4,9 px e −0,2;
/// `.s24`: 6,5 px sem espaçamento; `.s32`: 8,5 px e `.02em`, com `0 1px 1px rgba(0,0,0,.4)`). Os lados de fora
/// seguem o mais perto na proporção.
pub fn fonte_da_folha(texto: &str, lado: f32) -> (f32, f32, (f32, f32)) {
    let letras = texto.chars().count();
    if lado <= 20.0 {
        let k = lado / 16.0;
        let (t, esp) = if letras >= 4 { (4.9, -0.2) } else { (5.5, -0.15) };
        (t * k, esp * k, (0.5 * k, 0.45))
    } else if lado <= 28.0 {
        (6.5 * lado / 24.0, 0.0, (1.0, 0.4))
    } else {
        let t = 8.5 * lado / 32.0;
        (t, 0.02 * t, (1.0, 0.4))
    }
}

/// O texto branco 700 por cima do PNG, com a sombra pintada antes. Só com as fontes já instaladas (antes disso o
/// egui cairia).
fn pintar_rotulo(painter: &Painter, destino: Rect, lado: f32, rotulo: Rotulo) {
    if !fontes::instaladas(painter.ctx()) {
        return;
    }
    // o PNG pintado no tamanho dele pode ter até 1px a mais que o pedido: o texto acompanha
    let k = destino.width() / lado.max(0.01);
    let (texto, caixa, tamanho, espacamento, sombra) = match rotulo {
        Rotulo::Tipo(t) => {
            let glifo = glifo_do_tipo(t, lado);
            let (tamanho, esp) = fonte_do_glifo(glifo, lado);
            (glifo, destino, tamanho, esp, (1.0, 0.4))
        }
        Rotulo::Folha(texto) => {
            let r = retangulo_do_rotulo(destino.width(), Etiqueta::Grande).translate(destino.min.to_vec2());
            let (tamanho, esp, sombra) = fonte_da_folha(texto, lado);
            (texto, r, tamanho, esp, sombra)
        }
    };
    let mut formato = crate::pecas::texto::formato(fontes::fonte(tamanho * k, Peso::Pesado), Color32::WHITE);
    formato.extra_letter_spacing = espacamento * k;
    let g = painter.layout_job(LayoutJob::single_section(texto.to_owned(), formato));
    let canto = caixa.center() - g.size() / 2.0;
    painter.galley_with_override_text_color(canto + vec2(0.0, sombra.0 * k), g.clone(), rgba(0, 0, 0, sombra.1));
    painter.galley(canto, g, Color32::WHITE);
}

// ------------------------------------------------------------------ os traços

/// Os glifos de traço na grade de 24 (`stroke-width:1.6`, nunca abaixo de 1px).
fn pintar_traco(painter: &Painter, rect: Rect, icone: Icone, cor: Color32) {
    let k = rect.width() / 24.0;
    let origem = rect.min;
    let p = |x: f32, y: f32| origem + vec2(x, y) * k;
    let traco = Stroke::new((1.6 * k).max(1.0), cor);
    let linha = |pontos: Vec<Pos2>| {
        painter.add(PathShape::line(pontos, traco));
    };
    match icone {
        Icone::Fechar => {
            painter.line_segment([p(6.0, 6.0), p(18.0, 18.0)], traco);
            painter.line_segment([p(18.0, 6.0), p(6.0, 18.0)], traco);
        }
        Icone::Mais => {
            painter.line_segment([p(12.0, 5.0), p(12.0, 19.0)], traco);
            painter.line_segment([p(5.0, 12.0), p(19.0, 12.0)], traco);
        }
        Icone::SetaDireita => linha(vec![p(9.0, 6.0), p(15.0, 12.0), p(9.0, 18.0)]),
        Icone::SetaBaixo => linha(vec![p(6.0, 9.0), p(12.0, 15.0), p(18.0, 9.0)]),
        Icone::Marcado => linha(vec![p(5.0, 12.5), p(9.5, 17.0), p(19.0, 7.5)]),
        Icone::Destacar | Icone::Prender => {
            // a janela de trás (`M3 7a2 2 0 0 1 2-2h9…`), aberta onde a da frente a cobre, e a da frente
            // (`M10 11a2 2…`) com a barra de título; no Prender, tudo espelhado na vertical do meio
            let x = |v: f32| if icone == Icone::Prender { 24.0 - v } else { v };
            linha(vec![p(x(9.0), 18.0), p(x(4.0), 18.0), p(x(4.0), 6.0), p(x(15.0), 6.0), p(x(15.0), 8.0)]);
            let frente = Rect::from_two_pos(p(x(11.0), 10.0), p(x(20.0), 18.0));
            painter.rect_stroke(frente, 1.5 * k, traco, egui::StrokeKind::Middle);
            painter.line_segment([p(x(11.0), 12.2), p(x(20.0), 12.2)], traco);
        }
        _ => {}
    }
}

/// O arco de carregamento (`.ib .gira`: `M12 3.5a8.5 8.5 0 1 1-8.5 8.5` na grade de 24, `stroke-width:2` com as
/// pontas redondas) no quadrado centrado em `rect`, girado `giro` voltas (o `rotate` da animação de 1s; quem chama
/// conta o tempo e pede a repintura).
pub fn pintar_arco(painter: &Painter, rect: Rect, cor: Color32, giro: f32) {
    let lado = rect.width().min(rect.height());
    if lado <= 0.0 {
        return;
    }
    let k = lado / 24.0;
    let centro = rect.center();
    let raio = 8.5 * k;
    let largura = (2.0 * k).max(1.0);
    let inicio = -std::f32::consts::FRAC_PI_2 + giro * std::f32::consts::TAU;
    let passos = 28;
    let pontos: Vec<Pos2> = (0..=passos)
        .map(|i| {
            let a = inicio + 1.5 * std::f32::consts::PI * i as f32 / passos as f32;
            centro + vec2(a.cos(), a.sin()) * raio
        })
        .collect();
    for ponta in [pontos[0], pontos[passos]] {
        painter.circle_filled(ponta, largura / 2.0, cor);
    }
    painter.add(PathShape::line(pontos, PathStroke::new(largura, cor)));
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Os tamanhos em pixels de cada família, como o `icones_app.py` pede ao rasterizador.
    fn tamanhos_esperados(icone: Icone) -> &'static [u32] {
        const SIL: &[u32] = &[12, 14, 15, 16, 18, 20, 21, 24, 28, 32];
        const TRILHO: &[u32] = &[22, 33, 44];
        const MENU: &[u32] = &[24, 36, 48];
        const TABELAS: &[u32] = &[16, 20, 24, 30, 32, 40];
        const PARTES: &[u32] = &[16, 24, 32];
        const SECOES: &[u32] = &[20, 30, 40];
        const BANCO: &[u32] = &[16, 20, 24, 30, 32, 36, 40, 48];
        const PASTA: &[u32] = &[16, 24, 32, 36, 48];
        match icone {
            _ if icone.familia() == Familia::Silhueta => SIL,
            Icone::Inicio | Icone::Explorador | Icone::Historico => TRILHO,
            Icone::Preferencias => &[20, 22, 24, 30, 33, 36, 40, 44, 48],
            Icone::DocumentoSolido | Icone::NovoBancoSolido | Icone::DisqueteSolido => MENU,
            Icone::PastaSolida | Icone::Recentes => PASTA,
            Icone::Banco | Icone::BancoSoLeitura => BANCO,
            Icone::Tabela | Icone::TabelaSemRowid | Icone::TabelaVirtual | Icone::Visao | Icone::Sistema => TABELAS,
            Icone::Definicao | Icone::Consulta | Icone::Folha(TipoArquivo::Sql) => TABELAS,
            Icone::Dados | Icone::Estrutura | Icone::Indices | Icone::Indice | Icone::Gatilho => PARTES,
            Icone::Disco | Icone::Linhas => PARTES,
            Icone::TabelaGrande
            | Icone::VisaoGrande
            | Icone::IndiceGrande
            | Icone::GatilhoGrande
            | Icone::DiscoGrande
            | Icone::LinhasGrande => &[48, 72, 96],
            Icone::Folha(_) => PARTES,
            Icone::Geral | Icone::Aparencia | Icone::Exportacao => SECOES,
            Icone::Tipo(_) => &[16, 24, 32, 36, 48],
            Icone::Estado(_) => &[14, 16, 21, 24, 28, 32, 36, 48],
            _ => &[],
        }
    }

    #[test]
    fn todo_icone_tem_png_nos_tamanhos() {
        for icone in Icone::TODOS {
            let lados: Vec<u32> = icone.pngs().iter().map(|(l, _)| *l).collect();
            assert_eq!(lados, tamanhos_esperados(icone), "{icone:?}");
            assert!(lados.windows(2).all(|w| w[0] < w[1]), "{icone:?} fora de ordem");
        }
        // cada variante interna aparece no TODOS, e nenhum se repete
        for t in TipoColuna::TODOS {
            assert!(Icone::TODOS.contains(&Icone::Tipo(t)));
        }
        for a in TipoArquivo::TODOS {
            assert!(Icone::TODOS.contains(&Icone::Folha(a)));
        }
        for e in EstadoIcone::TODOS {
            assert!(Icone::TODOS.contains(&Icone::Estado(e)));
        }
        let unicos: std::collections::HashSet<Icone> = Icone::TODOS.iter().copied().collect();
        assert_eq!(unicos.len(), Icone::TODOS.len());
        assert!(Icone::TODOS.iter().all(|i| !i.nome().is_empty()));
    }

    #[test]
    fn pngs_dos_icones_abrem_no_tamanho_do_nome() {
        for (chave, lista) in chaves_png() {
            for &(lado, bytes) in lista {
                let (rgba, w, h) = crate::marca::decodificar_png(bytes).expect("png");
                assert_eq!((w, h), (lado, lado), "{chave}-{lado}");
                assert_eq!(rgba[3], 0, "canto de cima à esquerda opaco em {chave}-{lado}");
            }
        }
    }

    #[test]
    fn todo_png_da_pasta_esta_na_tabela() {
        let pasta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join("icones");
        let mut achados = Vec::new();
        for familia in std::fs::read_dir(&pasta).expect("assets/icones") {
            let familia = familia.expect("pasta");
            let nome_familia = familia.file_name().to_string_lossy().into_owned();
            for arquivo in std::fs::read_dir(familia.path()).expect("arquivos") {
                let nome = arquivo.expect("arquivo").file_name().to_string_lossy().into_owned();
                let (base, lado) = nome.strip_suffix(".png").and_then(|n| n.rsplit_once('-')).expect("<nome>-<px>.png");
                achados.push((format!("{nome_familia}/{base}"), lado.parse::<u32>().expect("px")));
            }
        }
        let mut tabela: Vec<(String, u32)> =
            chaves_png().flat_map(|(c, lista)| lista.iter().map(move |(l, _)| (c.to_owned(), *l))).collect();
        achados.sort();
        tabela.sort();
        assert_eq!(achados, tabela, "a pasta e a tabela gerada (rode icones_app.py --tabela)");
        // e cada chave da tabela é de algum ícone
        for (chave, _) in chaves_png() {
            assert!(Icone::TODOS.iter().any(|i| i.chave_png() == Some(chave)), "{chave} sem ícone");
        }
    }

    #[test]
    fn escolhe_o_png_pela_escala_dos_icones() {
        // a silhueta de 16 pt: a 100% o de 16, a 125% o de 20, a 150% o de 24, a 200% o de 32
        for (escala, px) in [(1.0, 16), (1.25, 20), (1.5, 24), (2.0, 32)] {
            assert_eq!(lado_do_png(Icone::Salvar, 16.0, escala), Some(px), "{escala}");
        }
        // 14 pt a 125% (17,5 px): o de 18, pintado no tamanho dele
        assert_eq!(lado_do_png(Icone::Editar, 14.0, 1.25), Some(18));
        // o sólido de 16 pt não tem o 1,25×: o de 24 reduzido
        assert_eq!(lado_do_png(Icone::Dados, 16.0, 1.25), Some(24));
        assert_eq!(lado_do_png(Icone::Tabela, 16.0, 1.25), Some(20));
        // o trilho: 22 pt a 150% e a 200%
        assert_eq!(lado_do_png(Icone::Explorador, 22.0, 1.5), Some(33));
        assert_eq!(lado_do_png(Icone::Explorador, 22.0, 2.0), Some(44));
        // maior que todos: o maior
        assert_eq!(lado_do_png(Icone::Banco, 48.0, 2.0), Some(48));
        assert_eq!(lado_do_png(Icone::Fechar, 16.0, 1.0), None);
        assert_eq!(escolher_png(&[], 16.0), None);
        // na tela: pixel a pixel quando o PNG está a até 1px; senão, reduzido; o canto nos pixels
        let r = retangulo_na_tela(pos2(10.3, 10.3), 14.0, 18, 1.25);
        assert!((r.width() * 1.25 - 18.0).abs() < 1e-4, "{r:?}");
        assert!((r.min.x * 1.25 - (r.min.x * 1.25).round()).abs() < 1e-3, "{r:?}");
        let r = retangulo_na_tela(pos2(10.0, 10.0), 16.0, 24, 1.25);
        assert!((r.width() * 1.25 - 20.0).abs() < 1e-4, "{r:?}");
    }

    #[test]
    fn desenho_da_etiqueta_pelo_tamanho_em_pontos() {
        // o glifo pelo lado em pontos, não pelos pixels: o de 16 pt a 200% continua "12"
        assert_eq!(glifo_do_tipo(TipoColuna::Inteiro, 16.0), "12");
        // a ficha da linha (prancheta 12) tem "12" e ",5" nas pastilhas de 24
        assert_eq!(glifo_do_tipo(TipoColuna::Inteiro, 24.0), "12");
        assert_eq!(glifo_do_tipo(TipoColuna::Real, 24.0), ",5");
        assert_eq!(glifo_do_tipo(TipoColuna::Inteiro, 48.0), "123");
        assert_eq!(glifo_do_tipo(TipoColuna::Real, 16.0), ",5");
        assert_eq!(glifo_do_tipo(TipoColuna::Real, 32.0), "1,5");
        assert_eq!(glifo_do_tipo(TipoColuna::ChavePrimaria, 48.0), "PK");
        assert_eq!(glifo_do_tipo(TipoColuna::Nulo, 16.0), "∅");
        // as fontes do `.sk.tipo.g1/g2/g3`
        assert_eq!(fonte_do_glifo("✓", 16.0).0, 10.0);
        assert_eq!(fonte_do_glifo("12", 16.0).0, 7.5);
        assert_eq!(fonte_do_glifo("123", 24.0).0, 9.5);
        assert_eq!(fonte_do_glifo("Aa", 24.0).0, 11.0);
        assert_eq!(fonte_do_glifo("PK", 32.0).0, 15.0);
        // a folha: 5,5 a 16 pt (4,9 com quatro letras), 6,5 a 24, 8,5 a 32
        assert_eq!(fonte_da_folha("SQL", 16.0).0, 5.5);
        assert_eq!(fonte_da_folha("XLSX", 16.0).0, 4.9);
        assert_eq!(fonte_da_folha("CSV", 24.0).0, 6.5);
        assert_eq!(fonte_da_folha("JSON", 32.0).0, 8.5);
        assert_eq!(fonte_da_folha("SQL", 16.0).2, (0.5, 0.45));
    }

    /// A pastilha `[]` da lista do JSON: a base e os tamanhos do `{}`, só o glifo e o nome mudam.
    #[test]
    fn tipo_lista_usa_a_base_do_json_com_o_glifo_dos_colchetes() {
        let (lista, json) = (Icone::Tipo(TipoColuna::Lista), Icone::Tipo(TipoColuna::Json));
        assert_eq!(lista.chave_png(), json.chave_png());
        assert_eq!(lista.pngs().len(), json.pngs().len());
        assert_eq!(lista.familia(), Familia::Solido);
        assert_eq!(glifo_do_tipo(TipoColuna::Lista, 16.0), "[]");
        assert_eq!(glifo_do_tipo(TipoColuna::Lista, 48.0), "[]");
        assert_eq!(glifo_do_tipo(TipoColuna::Json, 16.0), "{}");
        assert_eq!(TipoColuna::Lista.nome(), "Lista");
        assert_eq!(lista.nome(), "Lista");
        // nenhum tipo declarado do banco vira lista: ela só existe na estrutura do JSON
        assert!(TipoColuna::TODOS.contains(&TipoColuna::Lista));
        assert_ne!(TipoColuna::do_declarado("JSON", false, false), TipoColuna::Lista);
    }

    #[test]
    fn rotulo_txt_tem_tres_letras_com_a_fonte_normal() {
        // "TXT" tem três letras como "SQL" e "CSV": a mesma fonte (5,5 a 16 pt, 6,5 a 24, 8,5 a 32), sem a redução
        // das etiquetas de quatro letras
        assert_eq!(TipoArquivo::Txt.rotulo(), "TXT");
        assert_eq!(Icone::Folha(TipoArquivo::Txt).rotulo(), Some(Rotulo::Folha("TXT")));
        for lado in [16.0, 24.0, 32.0] {
            assert_eq!(fonte_da_folha("TXT", lado), fonte_da_folha("CSV", lado), "lado {lado}");
        }
        assert_eq!(fonte_da_folha("TXT", 16.0).0, 5.5);
        assert!(fonte_da_folha("XLSX", 16.0).0 < fonte_da_folha("TXT", 16.0).0);
        assert_eq!(Icone::Folha(TipoArquivo::Txt).pngs().len(), 3);
    }

    #[test]
    fn retangulo_do_rotulo_como_o_css() {
        let r = retangulo_do_rotulo(48.0, Etiqueta::Grande);
        let perto = |a: f32, b: f32| (a - b).abs() < 0.01;
        assert!(perto(r.left(), 2.016) && perto(r.top(), 21.0) && perto(r.width(), 39.984) && perto(r.height(), 21.0));
        let m = retangulo_do_rotulo(32.0, Etiqueta::Menor);
        assert!(perto(m.left(), 2.0) && perto(m.top(), 15.328) && perto(m.width(), 24.0) && perto(m.height(), 10.656));
    }

    #[test]
    fn pastilha_pelo_tipo_declarado() {
        use TipoColuna as T;
        for (tipo, pk, fk, esperado) in [
            ("INTEGER", true, false, T::ChavePrimaria),
            ("INTEGER", false, true, T::ChaveEstrangeira),
            ("INTEGER", true, true, T::ChavePrimaria),
            ("INTEGER", false, false, T::Inteiro),
            ("BIGINT", false, false, T::Inteiro),
            ("UNSIGNED BIG INT", false, false, T::Inteiro),
            ("REAL", false, false, T::Real),
            ("DOUBLE PRECISION", false, false, T::Real),
            ("NUMERIC(10,2)", false, false, T::Real),
            ("DECIMAL", false, false, T::Real),
            ("TEXT", false, false, T::Texto),
            ("varchar(20)", false, false, T::Texto),
            ("BLOB", false, false, T::Blob),
            ("boolean", false, false, T::Booleano),
            ("BOOL", false, false, T::Booleano),
            ("DATE", false, false, T::Data),
            ("DATETIME", false, false, T::Data),
            ("TIMESTAMP", false, false, T::Data),
            ("TIME", false, false, T::Data),
            ("JSON", false, false, T::Json),
            ("", false, false, T::Nulo),
            ("   ", false, false, T::Nulo),
        ] {
            assert_eq!(TipoColuna::do_declarado(tipo, pk, fk), esperado, "{tipo:?} pk={pk} fk={fk}");
        }
    }

    #[test]
    fn so_os_tracos_ficam_no_pintor() {
        let tracos: Vec<Icone> = Icone::TODOS.iter().copied().filter(|i| i.familia() == Familia::Traco).collect();
        assert_eq!(
            tracos,
            [
                Icone::Fechar,
                Icone::Mais,
                Icone::SetaDireita,
                Icone::SetaBaixo,
                Icone::Marcado,
                Icone::Destacar,
                Icone::Prender
            ]
        );
        for icone in Icone::TODOS {
            assert_eq!(icone.familia() == Familia::Traco, icone.pngs().is_empty(), "{icone:?}");
        }
        assert_eq!(Icone::Inicio.familia(), Familia::Solido);
        assert_eq!(Icone::Casa.familia(), Familia::Silhueta);
        assert_eq!(Icone::Tipo(TipoColuna::Texto).familia(), Familia::Solido);
        // só a folha e a pastilha levam texto por cima
        assert_eq!(Icone::Consulta.rotulo(), Some(Rotulo::Folha("SQL")));
        assert_eq!(Icone::Tabela.rotulo(), None);
    }

    #[test]
    fn cinza_do_css_tira_a_cor() {
        let mut px = [44u8, 111, 220, 255];
        cinza_css(&mut px, 1.0);
        assert!(px[0] == px[1] && px[1] == px[2], "{px:?}");
        let mut igual = [44u8, 111, 220, 255];
        cinza_css(&mut igual, 0.0);
        assert_eq!(igual, [44, 111, 220, 255]);
    }
}
