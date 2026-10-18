//! Regras puras e peças pequenas que as abas e as janelas de estrutura compartilham (texto da referência,
//! se dá para editar e por quê, condição do índice, CREATE formatado, script da definição, o nome sugerido do
//! índice).
//!
//! As peças de desenho daqui só existem porque o crate `interface` não tem a variante que estas telas usam
//! (o relatório do crate lista cada uma): o link em fonte monoespaçada (`.links-sql a code`), o `details.modelo`
//! com o nome em negrito e as ações coladas no `pre` (os gatilhos), o painel rebaixado (`.ref-detalhes`,
//! `.lista-indice`), o título de seção da janela (`h4.titulo-secao`), o fluxo que quebra linha medindo os itens
//! (`.checks-coluna`), o título e a contagem da faixa das partes e o título de lista do documento.

use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

use banco::{TabelaInfo, TipoObjeto};
use egui::{
    Align, Color32, CursorIcon, Id, Layout, Margin, Rect, Response, Sense, Shape, Ui, UiBuilder, WidgetInfo,
    WidgetType, pos2, vec2,
};
use interface::fontes::{self, Peso};
use interface::pecas::texto::{Trecho, codigo, galley, galley_cortado, texto_rico};
use interface::pintura;
use interface::tema::paleta;
use servico::TabelaCompleta;

/// Os tipos do `datalist` da coluna.
pub(crate) const TIPOS: [&str; 9] =
    ["INTEGER", "TEXT", "REAL", "NUMERIC", "BLOB", "DATE", "DATETIME", "BOOLEAN", "VARCHAR(255)"];
/// As colações do select (além de "Nenhuma").
pub(crate) const COLACOES: [&str; 3] = ["NOCASE", "RTRIM", "BINARY"];
/// As ações de ON DELETE / ON UPDATE, na ordem do menu.
pub(crate) const ACOES_FK: [&str; 5] = ["NO ACTION", "RESTRICT", "SET NULL", "SET DEFAULT", "CASCADE"];

/// A frase do toast de uma mudança de estrutura sem descrição.
pub(crate) const MSG_ESTRUTURA_ALTERADA: &str = "Estrutura alterada.";

// ------------------------------------------------------------------ regras

/// A estrutura da tabela pode ser mudada nesta sessão (o `podeEditar`).
pub fn pode_editar(info: &TabelaInfo, somente_leitura: bool) -> bool {
    info.esquema_editavel && !somente_leitura
}

/// Frase curta dizendo por que a estrutura está só em leitura (o `motivoSemEdicao`).
pub fn motivo_sem_edicao(completa: &TabelaCompleta, somente_leitura: bool) -> String {
    let info = &completa.info;
    if somente_leitura {
        return "O banco está aberto só para leitura; a estrutura não pode ser mudada.".into();
    }
    if let Some(a) = completa.aviso_esquema.as_deref().filter(|a| !a.is_empty()) {
        return a.to_string();
    }
    match info.tipo {
        TipoObjeto::View => {
            "Visão: a estrutura dela é o próprio SELECT, que se muda excluindo e criando de novo.".into()
        }
        TipoObjeto::Virtual => "Tabela virtual: as colunas vêm do módulo que a criou e não mudam por aqui.".into(),
        TipoObjeto::Shadow => "Tabela auxiliar de uma tabela virtual: o SQLite cuida da estrutura dela.".into(),
        TipoObjeto::Table if info.sistema => "Tabela interna do SQLite: a estrutura dela não pode ser mudada.".into(),
        TipoObjeto::Table => "A estrutura desta tabela não pode ser mudada por aqui.".into(),
    }
}

/// O texto da referência de uma chave estrangeira, igual nos cards Colunas e Chaves estrangeiras (o
/// `textoReferencia`): "→ clientes.id", "→ tabela (a, b)" ou só "→ tabela".
pub fn texto_referencia<S: AsRef<str>>(tabela: &str, colunas: &[Option<S>]) -> String {
    let lista: Vec<&str> =
        colunas.iter().filter_map(|c| c.as_ref().map(AsRef::as_ref)).filter(|c| !c.is_empty()).collect();
    match lista.as_slice() {
        [] => format!("→ {tabela}"),
        [uma] => format!("→ {tabela}.{uma}"),
        varias => format!("→ {tabela} ({})", varias.join(", ")),
    }
}

/// Pula um texto ou nome citado que começa em `bytes[i]` (`'…'`, `"…"`, `` `…` `` ou `[…]`, com as aspas
/// dobradas); devolve a posição do caractere que fecha (ou o fim).
fn pular_citado(bytes: &[u8], i: usize) -> usize {
    let abre = bytes[i];
    let fecha = if abre == b'[' { b']' } else { abre };
    let mut k = i + 1;
    while k < bytes.len() {
        if bytes[k] == fecha {
            if fecha != b']' && bytes.get(k + 1) == Some(&fecha) {
                k += 2;
                continue;
            }
            break;
        }
        k += 1;
    }
    k
}

fn e_citacao(b: u8) -> bool {
    matches!(b, b'\'' | b'"' | b'`' | b'[')
}

fn e_letra_de_palavra(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// A condição WHERE de um índice parcial, tirada do CREATE INDEX (o `condicaoIndice`): o que vem depois do
/// parêntese que fecha a lista de colunas, pulando textos e nomes citados.
pub fn condicao_indice(sql: &str) -> Option<String> {
    // os delimitadores são ASCII: varrer os bytes é seguro e as posições caem em fronteiras de caractere
    let bytes = sql.as_bytes();
    let mut profundidade = 0_i64;
    let mut abriu = false;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if e_citacao(c) {
            i = pular_citado(bytes, i) + 1;
            continue;
        }
        if c == b'(' {
            profundidade += 1;
            abriu = true;
        } else if c == b')' {
            profundidade -= 1;
            if abriu && profundidade == 0 {
                let resto = sql.get(i + 1..).unwrap_or("").trim();
                let comeca_com_where = resto.get(..5).is_some_and(|s| s.eq_ignore_ascii_case("where"))
                    && resto.get(5..).and_then(|r| r.chars().next()).is_none_or(|c| !e_letra_de_palavra(c));
                if !comeca_com_where {
                    return None;
                }
                let corpo = resto.get(5..).unwrap_or("").trim();
                let corpo = corpo.strip_suffix(';').unwrap_or(corpo).trim();
                return (!corpo.is_empty()).then(|| corpo.to_string());
            }
        }
        i += 1;
    }
    None
}

/// Divide o texto nas vírgulas de nível zero, pulando textos e nomes citados (o `dividirNoTopo`).
fn dividir_no_topo(texto: &str) -> Vec<&str> {
    let bytes = texto.as_bytes();
    let mut partes = Vec::new();
    let mut profundidade = 0_i64;
    let mut inicio = 0;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if e_citacao(c) {
            i = pular_citado(bytes, i);
        } else if c == b'(' {
            profundidade += 1;
        } else if c == b')' {
            profundidade -= 1;
        } else if c == b',' && profundidade == 0 {
            partes.push(texto.get(inicio..i).unwrap_or("").trim());
            inicio = i + 1;
        }
        i += 1;
    }
    partes.push(texto.get(inicio..).unwrap_or("").trim());
    partes
}

/// Posição do parêntese que fecha o que abre em `abre` (o `fechamento`).
fn fechamento(texto: &str, abre: usize) -> Option<usize> {
    let bytes = texto.as_bytes();
    let mut profundidade = 0_i64;
    let mut i = abre;
    while i < bytes.len() {
        let c = bytes[i];
        if e_citacao(c) {
            i = pular_citado(bytes, i);
        } else if c == b'(' {
            profundidade += 1;
        } else if c == b')' {
            profundidade -= 1;
            if profundidade == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// `^CREATE\s+(TEMP\s+|TEMPORARY\s+)?TABLE\b` sem diferenciar maiúsculas.
fn comeca_com_create_table(texto: &str) -> bool {
    fn palavra<'a>(resto: &'a str, alvo: &str) -> Option<&'a str> {
        let cabeca = resto.get(..alvo.len())?;
        if !cabeca.eq_ignore_ascii_case(alvo) {
            return None;
        }
        let depois = resto.get(alvo.len()..)?;
        depois.chars().next().is_none_or(|c| !e_letra_de_palavra(c)).then_some(depois)
    }
    fn espacos(resto: &str) -> Option<&str> {
        let sem = resto.trim_start();
        (sem.len() < resto.len()).then_some(sem)
    }
    let Some(resto) = palavra(texto, "CREATE").and_then(espacos) else { return false };
    let resto = ["TEMPORARY", "TEMP"].iter().find_map(|t| palavra(resto, t).and_then(espacos)).unwrap_or(resto);
    palavra(resto, "TABLE").is_some()
}

/// CREATE TABLE escrito numa linha só ganha uma coluna por linha; o que já tem quebras de linha fica como a
/// pessoa escreveu (o `formatarCreate`).
pub fn formatar_create(sql: &str) -> String {
    let texto = sql.trim();
    if texto.is_empty() || texto.contains('\n') || !comeca_com_create_table(texto) {
        return texto.to_string();
    }
    let Some(abre) = texto.find('(') else { return texto.to_string() };
    let Some(fecha) = fechamento(texto, abre) else { return texto.to_string() };
    let dentro = texto.get(abre + 1..fecha).unwrap_or("");
    let partes: Vec<&str> = dividir_no_topo(dentro).into_iter().filter(|p| !p.is_empty()).collect();
    format!(
        "{} (\n    {}\n){}",
        texto.get(..abre).unwrap_or("").trim_end(),
        partes.join(",\n    "),
        texto.get(fecha + 1..).unwrap_or("")
    )
}

fn com_ponto(sql: &str) -> String {
    let t = sql.trim();
    if t.ends_with(';') { t.to_string() } else { format!("{t};") }
}

/// O script da aba Definição SQL: criação, índices criados (os automáticos vêm da própria tabela) e gatilhos
/// (o `scriptDefinicao`).
pub fn script_definicao(info: &TabelaInfo) -> String {
    let mut partes = Vec::new();
    if let Some(sql) = info.sql.as_deref().filter(|s| !s.is_empty()) {
        partes.push(com_ponto(&if info.tipo == TipoObjeto::Table { formatar_create(sql) } else { sql.to_string() }));
    }
    for ind in &info.indices {
        if let Some(sql) = ind.sql.as_deref().filter(|s| !s.is_empty()) {
            partes.push(com_ponto(sql));
        }
    }
    for g in &info.gatilhos {
        if let Some(sql) = g.sql.as_deref().filter(|s| !s.is_empty()) {
            partes.push(com_ponto(sql));
        }
    }
    partes.join("\n\n")
}

/// O SQL de um gatilho como o card Gatilhos mostra e copia: sem espaços nas pontas e com ";" no fim.
pub(crate) fn sql_gatilho(sql: Option<&str>) -> String {
    format!("{};", sql.unwrap_or("").trim())
}

/// O nome sugerido de um índice (o `nomeSugerido`): `idx_<tabela>_<colunas>`, sem acentos, com "_" no lugar
/// de cada trecho que não é letra, número ou "_", em minúsculas.
pub fn nome_sugerido<S: AsRef<str>>(tabela: &str, colunas: &[S]) -> String {
    let mut partes = Vec::with_capacity(colunas.len() + 1);
    for p in std::iter::once(tabela).chain(colunas.iter().map(AsRef::as_ref)) {
        let mut limpo = String::with_capacity(p.len());
        let mut em_troca = false;
        for c in p.chars().filter_map(interface::formato::letra_sem_acento) {
            if e_letra_de_palavra(c) {
                limpo.push(c);
                em_troca = false;
            } else if !em_troca {
                limpo.push('_');
                em_troca = true;
            }
        }
        let parte = limpo.trim_matches('_').to_lowercase();
        if !parte.is_empty() {
            partes.push(parte);
        }
    }
    format!("idx_{}", partes.join("_"))
}

/// Um resumo (hash) de textos, para refazer um texto guardado só quando a tabela muda.
pub(crate) fn assinatura(partes: impl IntoIterator<Item = impl Hash>) -> u64 {
    let mut h = std::hash::DefaultHasher::new();
    for p in partes {
        p.hash(&mut h);
    }
    h.finish()
}

/// Opções de um select com o valor atual acrescentado quando não está na lista: devolve
/// os valores (o primeiro é "" quando há `vazio`) e o índice do escolhido (comparando sem diferenciar
/// maiúsculas; sem atual, o vazio).
pub(crate) fn opcoes_select(lista: &[&str], atual: Option<&str>, vazio: bool) -> (Vec<String>, usize) {
    let mut valores: Vec<String> = Vec::with_capacity(lista.len() + 2);
    if vazio {
        valores.push(String::new());
    }
    valores.extend(lista.iter().map(|v| (*v).to_string()));
    let atual = atual.filter(|a| !a.is_empty());
    if let Some(a) = atual
        && !lista.iter().any(|v| v.to_uppercase() == a.to_uppercase())
    {
        valores.push(a.to_string());
    }
    let escolhido = atual
        .and_then(|a| valores.iter().position(|v| !v.is_empty() && v.to_uppercase() == a.to_uppercase()))
        .unwrap_or(0);
    (valores, escolhido)
}

// ------------------------------------------------------------------ peças pequenas

/// O `details.modelo` de um gatilho: fio em cima (o primeiro sem fio e sem o padding de cima), `padding:12px
/// 0`, o `summary` com "▸"/"▾" em ouro e o nome em `strong`; aberto, o conteúdo vem 8px abaixo (o
/// `margin-top` do `pre`) e as ações coladas nele (a `.acoes-linha` não tem margem). O estado fica no contexto.
pub(crate) fn modelo_forte<R>(
    ui: &mut Ui,
    id: Id,
    nome: &str,
    primeiro: bool,
    conteudo: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let mut aberto: bool = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(false);
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let p = paleta(ui);
        let largura = ui.available_width();
        if !primeiro {
            let topo = ui.cursor().min;
            ui.painter().hline(topo.x..=topo.x + largura, topo.y + 0.5, egui::Stroke::new(1.0, p.hairline));
            ui.add_space(13.0);
        }
        let (rect, resposta) = ui.allocate_exact_size(vec2(largura, 19.5), Sense::click());
        if resposta.clicked() {
            aberto = !aberto;
            ui.ctx().data_mut(|d| d.insert_temp(id, aberto));
        }
        let gs = galley(ui, if aberto { "▾" } else { "▸" }, fontes::fonte(11.0, Peso::Normal), p.gold_text);
        let x_texto = rect.left() + gs.size().x + 8.0;
        let gt = galley_cortado(ui, nome, fontes::fonte(13.0, Peso::Pesado), p.fg, (rect.right() - x_texto).max(10.0));
        ui.painter().galley(pos2(rect.left(), rect.center().y - gs.size().y / 2.0), gs, p.gold_text);
        let largura_texto = gt.size().x;
        ui.painter().galley(pos2(x_texto, rect.center().y - gt.size().y / 2.0), gt, p.fg);
        if resposta.has_focus() && pintura::foco_visivel(ui.ctx()) {
            let alvo = Rect::from_min_max(rect.min, pos2(x_texto + largura_texto, rect.max.y));
            pintura::anel_foco(ui.painter(), p, alvo, 4.0);
        }
        if resposta.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        resposta.widget_info(|| WidgetInfo::selected(WidgetType::CollapsingHeader, true, aberto, nome));
        // a explicação de uso vai para a dica (nada de legenda no documento)
        let _ = resposta.on_hover_text(if aberto { "Esconder o SQL" } else { "Mostrar o SQL" });
        let inner = if aberto {
            ui.add_space(8.0);
            Some(conteudo(ui))
        } else {
            None
        };
        ui.add_space(12.0);
        inner
    })
    .inner
}

/// O painel rebaixado (`.ref-detalhes`, `.lista-indice`): fundo `--k-surface2`, anel `--k-trackRing`, o raio
/// único dos painéis ([`pintura::RAIO_PADRAO`], 10) e o conteúdo com a margem dada.
pub(crate) fn painel_rebaixado<R>(ui: &mut Ui, margem: Margin, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
    let p = paleta(ui);
    let lugar = ui.painter().add(Shape::Noop);
    let r = egui::Frame::NONE.inner_margin(margem).show(ui, |ui| {
        ui.set_width(ui.available_width());
        conteudo(ui)
    });
    let rect = r.response.rect;
    ui.painter().set(lugar, Shape::rect_filled(rect, pintura::RAIO_PADRAO, p.surface2));
    pintura::anel_rebaixado(ui.painter(), rect, pintura::RAIO_PADRAO, p.track_ring);
    r.inner
}

/// O título da prévia (o "SQL que será executado", o `.tit` do `.blk` das pranchetas 12b e 12c): 16px depois do
/// bloco de cima (o `gap:16px` do corpo da janela) e a linha de 15px do `.tit`.
pub(crate) fn titulo_previa(ui: &mut Ui, texto: &str) {
    ui.add_space(ESPACO_ENTRE_BLOCOS);
    interface::pecas::arranjo::linha(ui, 15.0, 0.0, |ui| interface::pecas::texto::h4(ui, texto));
}

/// O espaço entre os blocos do corpo de uma janela de edição (`gap:16px`).
pub(crate) const ESPACO_ENTRE_BLOCOS: f32 = 16.0;

/// Itens lado a lado que quebram linha, medidos antes (o `display:flex; flex-wrap:wrap; gap` das caixas da
/// janela de coluna): `larguras[i]` é a largura do item `i`, `altura` a da linha. Cada item é desenhado num
/// `Ui` do tamanho dele, centrado na vertical.
pub(crate) fn fluxo_medido(
    ui: &mut Ui,
    larguras: &[f32],
    altura: f32,
    espaco_x: f32,
    espaco_y: f32,
    mut item: impl FnMut(&mut Ui, usize),
) {
    let largura = ui.available_width();
    let topo = ui.cursor().min;
    let (mut x, mut y) = (0.0_f32, 0.0_f32);
    let id = ui.next_auto_id();
    for (i, &w) in larguras.iter().enumerate() {
        if x > 0.0 && x + w > largura + 0.5 {
            x = 0.0;
            y += altura + espaco_y;
        }
        let rect = Rect::from_min_size(topo + vec2(x, y), vec2(w, altura));
        let mut filho = ui.new_child(
            UiBuilder::new().id_salt(id.with(i)).max_rect(rect).layout(Layout::left_to_right(Align::Center)),
        );
        filho.spacing_mut().item_spacing.x = 6.0;
        item(&mut filho, i);
        x += w + espaco_x;
    }
    let altura_total = if larguras.is_empty() { 0.0 } else { y + altura };
    ui.allocate_rect(Rect::from_min_size(topo, vec2(largura, altura_total)), Sense::hover());
    ui.skip_ahead_auto_ids(1);
}

/// Largura de uma `.tag` (padding 4px 11px, 11px / 600): a mesma conta de `interface::pecas::rotulos::tag`.
pub(crate) fn largura_tag(ui: &Ui, texto: &str) -> f32 {
    (galley(ui, texto, fontes::fonte(11.0, Peso::Medio), Color32::PLACEHOLDER).size().x + 22.0).ceil()
}

/// Largura de um `label.check` (caixa de 16px, 8px e o texto).
pub(crate) fn largura_check(ui: &Ui, texto: &str, tamanho: f32) -> f32 {
    let g = galley(ui, texto, fontes::fonte(tamanho, Peso::Normal), Color32::PLACEHOLDER);
    (16.0 + 8.0 + g.size().x).ceil()
}

/// `code` numa célula ou numa lista: o `code` do kit numa linha quando cabe em `largura_max`; senão o texto
/// quebra (o `white-space:pre-wrap; overflow-wrap:anywhere` das restrições e das condições).
pub(crate) fn codigo_quebravel(ui: &mut Ui, texto: &str, largura_max: f32) -> Response {
    let p = paleta(ui);
    let largura = galley(ui, texto, fontes::fonte_mono(11.5), Color32::PLACEHOLDER).size().x + 12.0;
    if largura <= largura_max.min(ui.available_width()) {
        codigo(ui, texto)
    } else {
        ui.scope(|ui| {
            ui.set_max_width(largura_max.min(ui.available_width()));
            texto_rico(ui, &[Trecho::Codigo(texto)], 11.5, p.fg, p.fg).resposta
        })
        .inner
    }
}

/// O que a faixa de contexto das partes diz (prancheta 6): o título em 600/13px (`strong`) e a contagem em `--t3`
/// ("Colunas" · "7 colunas"). A faixa já é uma linha com `gap:8px`.
pub(crate) fn titulo_da_faixa(ui: &mut Ui, titulo: &str, contagem: &str) {
    let t3 = paleta(ui).estudio.t3;
    interface::pecas::texto::titulo_lista(ui, titulo);
    if !contagem.is_empty() {
        interface::pecas::texto::texto(ui, contagem, 12.0, t3);
    }
}

/// O título de uma lista no corpo do documento (o `h2` 13/600 de "Tabelas" na Visão geral, com 8px até a lista).
pub(crate) fn titulo_lista(ui: &mut Ui, titulo: &str) {
    interface::pecas::texto::titulo_lista(ui, titulo);
    ui.add_space(ESPACO_TITULO_LISTA);
}

/// O título de uma lista com a contagem em `--t3` depois dele ("Gatilhos" · "2 gatilhos").
pub(crate) fn titulo_lista_com_contagem(ui: &mut Ui, titulo: &str, contagem: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        titulo_da_faixa(ui, titulo, contagem);
    });
    ui.add_space(ESPACO_TITULO_LISTA);
}

/// O espaço entre o título de uma lista e ela (`margin-bottom:8px`).
const ESPACO_TITULO_LISTA: f32 = 8.0;

/// Um número novo para cada aba criada: entra no id do que abre e fecha nela (a zona de risco, os modelos dos
/// gatilhos), que assim nasce fechado em cada aba nova.
pub(crate) fn nova_instancia() -> u64 {
    static INSTANCIAS: AtomicU64 = AtomicU64::new(0);
    INSTANCIAS.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn referencia_no_mesmo_formato_dos_dois_cards() {
        assert_eq!(texto_referencia("clientes", &[Some("id")]), "→ clientes.id");
        assert_eq!(texto_referencia::<&str>("clientes", &[None]), "→ clientes");
        assert_eq!(texto_referencia("t", &[Some("a"), None, Some("b")]), "→ t (a, b)");
        assert_eq!(texto_referencia("t", &[Some("")]), "→ t");
    }

    #[test]
    fn condicao_do_indice_parcial() {
        assert_eq!(
            condicao_indice("CREATE UNIQUE INDEX idx ON clientes (nome, cidade) WHERE ativo = 1").as_deref(),
            Some("ativo = 1")
        );
        assert_eq!(condicao_indice("CREATE INDEX i ON t (a) where (b > 2);").as_deref(), Some("(b > 2)"));
        assert_eq!(condicao_indice("CREATE INDEX i ON t (a)"), None);
        // nomes citados com "where" e parênteses não confundem
        assert_eq!(
            condicao_indice("CREATE INDEX i ON t (\"a) where\", [b(]) WHERE \"x\" = ')'").as_deref(),
            Some("\"x\" = ')'")
        );
        assert_eq!(condicao_indice("CREATE INDEX i ON t (a) WHEREVER"), None);
        assert_eq!(condicao_indice("CREATE INDEX i ON t (a) WHERE ;"), None);
        assert_eq!(condicao_indice(""), None);
    }

    #[test]
    fn create_numa_linha_ganha_uma_coluna_por_linha() {
        assert_eq!(
            formatar_create(
                "CREATE TABLE pedidos (id INTEGER PRIMARY KEY, total REAL DEFAULT (0), nota TEXT CHECK (nota <> 'a,b'))"
            ),
            "CREATE TABLE pedidos (\n    id INTEGER PRIMARY KEY,\n    total REAL DEFAULT (0),\n    nota TEXT CHECK (nota <> 'a,b')\n)"
        );
        assert_eq!(
            formatar_create("create temp table t(a, b) WITHOUT ROWID"),
            "create temp table t (\n    a,\n    b\n) WITHOUT ROWID"
        );
        // com quebras de linha, fica como a pessoa escreveu
        let escrito = "CREATE TABLE t (\n  a,\n  b\n)";
        assert_eq!(formatar_create(escrito), escrito);
        assert_eq!(formatar_create("CREATE VIEW v AS SELECT 1"), "CREATE VIEW v AS SELECT 1");
        assert_eq!(formatar_create("CREATE TABLEX t (a)"), "CREATE TABLEX t (a)");
        assert_eq!(formatar_create("CREATE TABLE t (a"), "CREATE TABLE t (a");
    }

    #[test]
    fn nome_sugerido_sem_acentos_e_em_minusculas() {
        assert_eq!(nome_sugerido::<&str>("pedidos", &[]), "idx_pedidos");
        assert_eq!(nome_sugerido("eventos", &["tipo", "momento"]), "idx_eventos_tipo_momento");
        assert_eq!(nome_sugerido("Tabela com espaço", &["Preço Unitário"]), "idx_tabela_com_espaco_preco_unitario");
        assert_eq!(nome_sugerido("__x__", &["--", "a-b"]), "idx_x_a_b");
        assert_eq!(nome_sugerido("ação", &["São Paulo"]), "idx_acao_sao_paulo");
    }

    #[test]
    fn opcoes_com_o_atual_que_nao_esta_na_lista() {
        let (v, i) = opcoes_select(&ACOES_FK, Some("cascade"), true);
        assert_eq!(v.len(), 6);
        assert_eq!(v[i], "CASCADE");
        let (v, i) = opcoes_select(&COLACOES, Some("utf8_ci"), true);
        assert_eq!(v.last().map(String::as_str), Some("utf8_ci"));
        assert_eq!(i, v.len() - 1);
        let (_, i) = opcoes_select(&COLACOES, None, true);
        assert_eq!(i, 0);
    }
}
