//! Fontes do DataBox: as do sistema, lidas em tempo de execução e nunca embutidas.
//!
//! - Interface: Segoe UI nos pesos que as pranchetas usam — Semilight 350 (números grandes, caminhos), Regular
//!   400 (corpo), Semibold 600 (títulos, botões, cabeçalhos), Bold 700 (etiquetas) e Italic 400 (o "NULL" e o
//!   "vazio" das células). Em outro sistema (o porte para Linux), Noto Sans ou DejaVu Sans; sem nenhuma, a
//!   Ubuntu-Light que vem com o egui.
//! - Monoespaçada (só dentro dos editores de SQL e JSON e no SQL das janelas): Consolas, senão Cascadia Mono; no
//!   Linux, DejaVu Sans Mono; sem nenhuma, a Hack do egui. A pessoa troca a família nas Preferências entre as
//!   candidatas que existem nesta máquina ([`familias_mono_disponiveis`]: Consolas, Cascadia Mono, Courier New,
//!   Lucida Console), com [`trocar_mono`] (o arquivo é lido por quem chama, com [`ler_familia_mono`], fora da
//!   thread da interface) e [`restaurar_mono`].
//! - O "i" do balão de ajuda: Georgia itálico negrito, senão a itálica da interface.
//! - Símbolos que a fonte da interface não tem (✕, ▸, ▾, ▲, ▼, →): Segoe UI Symbol (fora do Windows, DejaVu Sans
//!   ou Noto Sans Symbols2) e, por último, as do egui.
//!
//! As fontes são lidas uma vez, por [`instalar`], que o [`crate::tema::aplicar`] chama antes do primeiro quadro.

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use egui::{FontData, FontDefinitions, FontFamily, FontId};

/// Peso (e estilo) da fonte da interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Peso {
    /// 350 (Semilight): números grandes, caminhos, tipos.
    Leve,
    /// 400: corpo, campos, células.
    Normal,
    /// 600 (Semibold): títulos, botões, cabeçalhos de grade, rótulos de seção.
    Medio,
    /// 700 (Bold): `strong`, etiquetas, a extensão dos ícones de arquivo.
    Pesado,
    /// 400 itálico: `span.nulo`, `span.vazio-str`.
    Italico,
}

const NOME_LEVE: &str = "Interface 350";
const NOME_NORMAL: &str = "Interface 400";
const NOME_MEDIO: &str = "Interface 600";
const NOME_PESADO: &str = "Interface 700";
const NOME_ITALICO: &str = "Interface 400 itálico";
const NOME_AJUDA: &str = "Interface ajuda";
const NOME_MONO: &str = "Interface mono";
const NOME_SIMBOLOS: &str = "Interface símbolos";

static FAMILIA_LEVE: LazyLock<FontFamily> = LazyLock::new(|| FontFamily::Name(Arc::from(NOME_LEVE)));
static FAMILIA_MEDIO: LazyLock<FontFamily> = LazyLock::new(|| FontFamily::Name(Arc::from(NOME_MEDIO)));
static FAMILIA_PESADO: LazyLock<FontFamily> = LazyLock::new(|| FontFamily::Name(Arc::from(NOME_PESADO)));
static FAMILIA_ITALICO: LazyLock<FontFamily> = LazyLock::new(|| FontFamily::Name(Arc::from(NOME_ITALICO)));
static FAMILIA_AJUDA: LazyLock<FontFamily> = LazyLock::new(|| FontFamily::Name(Arc::from(NOME_AJUDA)));

/// A família do egui para um peso. O peso 400 é a `FontFamily::Proportional` (o padrão de todo texto).
pub fn familia(peso: Peso) -> FontFamily {
    match peso {
        Peso::Leve => FAMILIA_LEVE.clone(),
        Peso::Normal => FontFamily::Proportional,
        Peso::Medio => FAMILIA_MEDIO.clone(),
        Peso::Pesado => FAMILIA_PESADO.clone(),
        Peso::Italico => FAMILIA_ITALICO.clone(),
    }
}

/// A fonte da interface no tamanho (em pontos = px do CSS) e peso pedidos.
pub fn fonte(tamanho: f32, peso: Peso) -> FontId {
    FontId::new(tamanho, familia(peso))
}

/// A monoespaçada (só nos editores de SQL e JSON) no tamanho pedido.
pub fn fonte_mono(tamanho: f32) -> FontId {
    FontId::monospace(tamanho)
}

/// A fonte do "i" do balão de ajuda (Georgia itálico negrito, ou a itálica da interface).
pub fn fonte_ajuda(tamanho: f32) -> FontId {
    FontId::new(tamanho, FAMILIA_AJUDA.clone())
}

/// Confere o cabeçalho de um arquivo de fonte antes de entregá-lo ao egui, que entra em pânico com arquivo
/// inválido: versão sfnt TrueType (`00 01 00 00` ou `true`) ou CFF (`OTTO`), diretório de tabelas inteiro dentro
/// do arquivo e as tabelas `cmap`, `head`, `hhea`, `hmtx` e `maxp` presentes e dentro do arquivo.
pub fn fonte_valida(bytes: &[u8]) -> bool {
    fn u16_em(b: &[u8], i: usize) -> Option<usize> {
        Some(u16::from_be_bytes([*b.get(i)?, *b.get(i + 1)?]) as usize)
    }
    fn u32_em(b: &[u8], i: usize) -> Option<usize> {
        Some(u32::from_be_bytes([*b.get(i)?, *b.get(i + 1)?, *b.get(i + 2)?, *b.get(i + 3)?]) as usize)
    }
    let Some(versao) = bytes.get(0..4) else { return false };
    if versao != [0, 1, 0, 0] && versao != b"OTTO" && versao != b"true" {
        return false;
    }
    let Some(n_tabelas) = u16_em(bytes, 4) else { return false };
    if n_tabelas == 0 || 12 + n_tabelas * 16 > bytes.len() {
        return false;
    }
    let mut achadas = 0;
    for i in 0..n_tabelas {
        let base = 12 + i * 16;
        let (Some(tag), Some(inicio), Some(tamanho)) =
            (bytes.get(base..base + 4), u32_em(bytes, base + 8), u32_em(bytes, base + 12))
        else {
            return false;
        };
        if inicio.checked_add(tamanho).is_none_or(|fim| fim > bytes.len()) {
            return false;
        }
        if [b"cmap", b"head", b"hhea", b"hmtx", b"maxp"].iter().any(|t| tag == t.as_slice()) {
            achadas += 1;
        }
    }
    achadas == 5
}

/// Lê a primeira fonte válida da lista (caminhos completos). Erro de leitura ou arquivo inválido: a próxima.
fn ler_primeira(caminhos: &[PathBuf]) -> Option<Vec<u8>> {
    caminhos.iter().find_map(|c| std::fs::read(c).ok().filter(|b| fonte_valida(b)))
}

/// As pastas onde procurar fontes, na ordem.
fn pastas_de_fontes() -> Vec<PathBuf> {
    let mut pastas = Vec::new();
    if cfg!(windows) {
        let windir = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Windows"));
        pastas.push(windir.join("Fonts"));
        // fontes instaladas só para a pessoa (sem administrador)
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            pastas.push(Path::new(&local).join("Microsoft").join("Windows").join("Fonts"));
        }
    } else {
        // as pastas mais comuns das distribuições (Debian/Ubuntu e Fedora), para o porte futuro
        for p in [
            "/usr/share/fonts/truetype/noto",
            "/usr/share/fonts/noto",
            "/usr/share/fonts/google-noto",
            "/usr/share/fonts/truetype/dejavu",
            "/usr/share/fonts/dejavu-sans-fonts",
            "/usr/share/fonts/dejavu-sans-mono-fonts",
        ] {
            pastas.push(PathBuf::from(p));
        }
    }
    pastas
}

fn candidatos(nomes: &[&str]) -> Vec<PathBuf> {
    let pastas = pastas_de_fontes();
    nomes.iter().flat_map(|n| pastas.iter().map(move |p| p.join(n))).collect()
}

/// As fontes do sistema que a interface pede e que existem nesta máquina.
#[derive(Default)]
struct FontesDoSistema {
    leve: Option<Vec<u8>>,
    normal: Option<Vec<u8>>,
    medio: Option<Vec<u8>>,
    pesado: Option<Vec<u8>>,
    italico: Option<Vec<u8>>,
    mono: Option<Vec<u8>>,
    ajuda: Option<Vec<u8>>,
    simbolos: Option<Vec<u8>>,
}

fn ler_fontes_do_sistema() -> FontesDoSistema {
    FontesDoSistema {
        leve: ler_primeira(&candidatos(&["segoeuisl.ttf", "NotoSans-Light.ttf", "DejaVuSans-ExtraLight.ttf"])),
        normal: ler_primeira(&candidatos(&["segoeui.ttf", "NotoSans-Regular.ttf", "DejaVuSans.ttf"])),
        medio: ler_primeira(&candidatos(&["seguisb.ttf", "NotoSans-SemiBold.ttf", "DejaVuSans-Bold.ttf"])),
        pesado: ler_primeira(&candidatos(&["segoeuib.ttf", "NotoSans-Bold.ttf", "DejaVuSans-Bold.ttf"])),
        italico: ler_primeira(&candidatos(&["segoeuii.ttf", "NotoSans-Italic.ttf", "DejaVuSans-Oblique.ttf"])),
        // a Consolas é a padrão do código (decisão 11 da etapa 3); a Cascadia fica de reserva
        mono: ler_primeira(&candidatos(&[
            "consola.ttf",
            "CascadiaMono.ttf",
            "CascadiaMono-Regular.ttf",
            "DejaVuSansMono.ttf",
        ])),
        ajuda: ler_primeira(&candidatos(&["georgiaz.ttf"])),
        // fora do Windows não há Segoe UI Symbol: a DejaVu Sans tem as setas e triângulos que a Noto Sans não tem
        simbolos: ler_primeira(&candidatos(&["seguisym.ttf", "DejaVuSans.ttf", "NotoSansSymbols2-Regular.ttf"])),
    }
}

/// As definições de fonte completas, com as fontes do sistema lidas do disco agora (~5 MB).
pub fn definicoes() -> FontDefinitions {
    definicoes_com(ler_fontes_do_sistema())
}

fn id_instaladas() -> egui::Id {
    egui::Id::new("databox::fontes::instaladas")
}

/// Lê as fontes do sistema e as instala no contexto (valem a partir da passada seguinte). Chamado por
/// [`crate::tema::aplicar`] na primeira vez; as chamadas seguintes não fazem nada.
pub fn instalar(ctx: &egui::Context) {
    if ctx.data(|d| d.get_temp::<bool>(id_instaladas())).unwrap_or(false) {
        return;
    }
    let defs = definicoes();
    // a base da troca da monoespaçada: as definições pedidas agora só valem na passada seguinte
    ctx.data_mut(|d| d.insert_temp(id_base(), Arc::new(defs.clone())));
    ctx.set_fonts(defs);
    ctx.data_mut(|d| d.insert_temp(id_instaladas(), true));
}

// ---- a família do código ---------------------------------------------------------------------------

/// A família monoespaçada padrão do código (a de `prefs::Aparencia::fonte_codigo` None).
pub const FAMILIA_MONO_PADRAO: &str = "Consolas";

/// As candidatas da fonte do código, com os arquivos de cada uma (decisão 11 da etapa 3). Fora do Windows, a
/// DejaVu Sans Mono.
const CANDIDATAS_MONO: [(&str, &[&str]); 5] = [
    ("Consolas", &["consola.ttf"]),
    ("Cascadia Mono", &["CascadiaMono.ttf", "CascadiaMono-Regular.ttf"]),
    ("Courier New", &["cour.ttf"]),
    ("Lucida Console", &["lucon.ttf"]),
    ("DejaVu Sans Mono", &["DejaVuSansMono.ttf"]),
];

/// Uma família monoespaçada que existe nesta máquina, com o arquivo dela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamiliaMono {
    pub nome: &'static str,
    pub arquivo: PathBuf,
}

/// As candidatas que existem nas pastas de fontes do sistema (e nas da pessoa), na ordem das candidatas. Só um
/// `stat` por arquivo: nada é lido.
pub fn familias_mono_disponiveis() -> Vec<FamiliaMono> {
    familias_mono_em(&pastas_de_fontes())
}

/// O [`familias_mono_disponiveis`] em outras pastas (os testes usam uma pasta falsa).
pub fn familias_mono_em(pastas: &[PathBuf]) -> Vec<FamiliaMono> {
    CANDIDATAS_MONO
        .iter()
        .filter_map(|(nome, arquivos)| {
            arquivos
                .iter()
                .flat_map(|a| pastas.iter().map(move |p| p.join(a)))
                .find(|c| c.is_file())
                .map(|arquivo| FamiliaMono { nome, arquivo })
        })
        .collect()
}

/// Lê o arquivo da família e confere que é uma fonte ([`fonte_valida`]). Toca no disco: chame fora da thread da
/// interface.
pub fn ler_familia_mono(familia: &FamiliaMono) -> Option<Vec<u8>> {
    std::fs::read(&familia.arquivo).ok().filter(|b| fonte_valida(b))
}

const NOME_MONO_ESCOLHIDA: &str = "Interface mono escolhida";

fn id_base() -> egui::Id {
    egui::Id::new("databox::fontes::base")
}

fn id_escolhida() -> egui::Id {
    egui::Id::new("databox::fontes::mono_escolhida")
}

/// As definições de partida (as de [`instalar`]; sem elas, as que o contexto usa agora), sem a família escolhida.
fn base(ctx: &egui::Context) -> FontDefinitions {
    let mut defs = ctx
        .data(|d| d.get_temp::<Arc<FontDefinitions>>(id_base()))
        .map_or_else(|| ctx.fonts(|f| f.definitions().clone()), |b| (*b).clone());
    defs.font_data.remove(NOME_MONO_ESCOLHIDA);
    for cadeia in defs.families.values_mut() {
        cadeia.retain(|n| n != NOME_MONO_ESCOLHIDA);
    }
    defs
}

/// As definições de `base` com `bytes` à frente da cadeia monoespaçada (a família padrão e as do egui ficam de
/// reserva, para os glifos que a escolhida não tiver).
fn definicoes_com_mono(mut defs: FontDefinitions, bytes: Vec<u8>) -> FontDefinitions {
    let ajuste = ajuste_de_linha(&bytes);
    defs.font_data.insert(NOME_MONO_ESCOLHIDA.to_owned(), Arc::new(FontData::from_owned(bytes).tweak(ajuste)));
    let cadeia = defs.families.entry(FontFamily::Monospace).or_default();
    cadeia.retain(|n| n != NOME_MONO_ESCOLHIDA);
    cadeia.insert(0, NOME_MONO_ESCOLHIDA.to_owned());
    defs
}

/// Põe a família `nome` (os `bytes` do arquivo dela, de [`ler_familia_mono`]) no código: editores, `TextoPre::sql`
/// e os números das linhas. Vale a partir da passada seguinte (regra do `set_fonts`, que refaz o atlas: um quadro
/// mais lento). Arquivo que não é fonte → false, nada muda.
pub fn trocar_mono(ctx: &egui::Context, bytes: Vec<u8>, nome: &str) -> bool {
    if !fonte_valida(&bytes) {
        return false;
    }
    ctx.set_fonts(definicoes_com_mono(base(ctx), bytes));
    ctx.data_mut(|d| d.insert_temp(id_escolhida(), Some(nome.to_owned())));
    true
}

/// Volta à família padrão do código (a das definições de [`instalar`]).
pub fn restaurar_mono(ctx: &egui::Context) {
    if mono_escolhida(ctx).is_none() {
        return;
    }
    ctx.set_fonts(base(ctx));
    ctx.data_mut(|d| d.insert_temp::<Option<String>>(id_escolhida(), None));
}

/// A família trocada por [`trocar_mono`] (None: a padrão).
pub fn mono_escolhida(ctx: &egui::Context) -> Option<String> {
    ctx.data(|d| d.get_temp::<Option<String>>(id_escolhida())).flatten()
}

/// As fontes do sistema já foram lidas e pedidas ao contexto? (Valem a partir da passada seguinte.)
pub fn do_sistema_carregadas(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(id_instaladas())).unwrap_or(false)
}

/// Métricas verticais da tabela `hhea`, em frações do corpo: (ascendente, descendente positivo, entrelinha).
pub fn metricas_verticais(bytes: &[u8]) -> Option<(f32, f32, f32)> {
    let u16_em = |i: usize| -> Option<u16> { Some(u16::from_be_bytes([*bytes.get(i)?, *bytes.get(i + 1)?])) };
    let u32_em = |i: usize| -> Option<usize> {
        Some(u32::from_be_bytes([*bytes.get(i)?, *bytes.get(i + 1)?, *bytes.get(i + 2)?, *bytes.get(i + 3)?]) as usize)
    };
    let n = usize::from(u16_em(4)?);
    let mut head = None;
    let mut hhea = None;
    for i in 0..n {
        let base = 12 + i * 16;
        let tag = bytes.get(base..base + 4)?;
        let inicio = u32_em(base + 8)?;
        if tag == b"head" {
            head = Some(inicio);
        } else if tag == b"hhea" {
            hhea = Some(inicio);
        }
    }
    let upm = f32::from(u16_em(head? + 18)?);
    let h = hhea?;
    let asc = f32::from(u16_em(h + 4)? as i16);
    let desc = -f32::from(u16_em(h + 6)? as i16);
    let gap = f32::from(u16_em(h + 8)? as i16);
    (upm > 0.0).then(|| (asc / upm, desc / upm, gap / upm))
}

/// Quanto descer os glifos (fração do corpo) para o texto centrado numa caixa ficar onde o navegador o põe.
/// O egui põe a linha de base a um ascendente do alto da linha e a entrelinha (`lineGap`) inteira embaixo; o
/// CSS reparte a sobra igualmente em cima e embaixo. A diferença é metade da entrelinha.
fn ajuste_de_linha(bytes: &[u8]) -> egui::FontTweak {
    let fator = metricas_verticais(bytes).map_or(0.0, |(_, _, gap)| gap / 2.0);
    egui::FontTweak { y_offset_factor: fator, ..Default::default() }
}

fn definicoes_com(sistema: FontesDoSistema) -> FontDefinitions {
    // As fontes do egui (Ubuntu-Light, Hack, emojis) ficam como último recurso.
    let mut defs = FontDefinitions::default();
    let reserva_proporcional: Vec<String> = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let reserva_mono: Vec<String> = defs.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();

    // a métrica de referência para alinhar os símbolos é a da fonte normal da interface
    let referencia = sistema.normal.clone();
    let mut inserir = |nome: &str, bytes: Option<Vec<u8>>| -> Option<String> {
        let b = bytes?;
        let ajuste = ajuste_de_linha(&b);
        defs.font_data.insert(nome.to_owned(), Arc::new(FontData::from_owned(b).tweak(ajuste)));
        Some(nome.to_owned())
    };
    let normal = inserir(NOME_NORMAL, sistema.normal);
    let leve = inserir(NOME_LEVE, sistema.leve).or_else(|| normal.clone());
    let medio = inserir(NOME_MEDIO, sistema.medio).or_else(|| normal.clone());
    let pesado = inserir(NOME_PESADO, sistema.pesado).or_else(|| medio.clone());
    let italico = inserir(NOME_ITALICO, sistema.italico).or_else(|| normal.clone());
    let ajuda = inserir(NOME_AJUDA, sistema.ajuda).or_else(|| italico.clone());
    let mono = inserir(NOME_MONO, sistema.mono);

    let mut simbolos = Vec::new();
    if let Some(b) = sistema.simbolos {
        // Um glifo de reserva fica na linha de base que o egui calcula para fontes misturadas (o ascendente
        // dele, mais metade da diferença das alturas de linha); o deslocamento põe essa linha de base na da
        // fonte da interface. Sem isto, setas e triângulos da Segoe UI Symbol ficam fora da linha do texto.
        let ajuste = match (referencia.as_deref().and_then(metricas_verticais), metricas_verticais(&b)) {
            (Some((a_asc, a_desc, a_gap)), Some((s_asc, s_desc, s_gap))) => {
                let altura_a = a_asc + a_desc + a_gap;
                let altura_s = s_asc + s_desc + s_gap;
                a_asc + a_gap / 2.0 - s_asc - 0.5 * (altura_a - altura_s)
            }
            _ => 0.0,
        };
        let ajuste = egui::FontTweak { y_offset_factor: ajuste, ..Default::default() };
        defs.font_data.insert(NOME_SIMBOLOS.to_owned(), Arc::new(FontData::from_owned(b).tweak(ajuste)));
        simbolos.push(NOME_SIMBOLOS.to_owned());
    }

    let cadeia = |principal: &Option<String>| -> Vec<String> {
        let mut v: Vec<String> = principal.iter().cloned().collect();
        v.extend(simbolos.iter().cloned());
        v.extend(reserva_proporcional.iter().cloned());
        v
    };
    defs.families.insert(FontFamily::Proportional, cadeia(&normal));
    defs.families.insert(FAMILIA_LEVE.clone(), cadeia(&leve));
    defs.families.insert(FAMILIA_MEDIO.clone(), cadeia(&medio));
    defs.families.insert(FAMILIA_PESADO.clone(), cadeia(&pesado));
    defs.families.insert(FAMILIA_ITALICO.clone(), cadeia(&italico));
    defs.families.insert(FAMILIA_AJUDA.clone(), cadeia(&ajuda));

    let mut cadeia_mono: Vec<String> = mono.into_iter().collect();
    cadeia_mono.extend(reserva_mono);
    cadeia_mono.extend(simbolos.iter().cloned());
    defs.families.insert(FontFamily::Monospace, cadeia_mono);
    defs
}

/// As famílias nomeadas já estão instaladas no contexto? (As fontes trocadas por `Context::set_fonts` só valem a
/// partir da passada seguinte; antes disso, desenhar com uma família nomeada derrubaria o egui.)
pub fn instaladas(ctx: &egui::Context) -> bool {
    ctx.fonts(|f| f.definitions().families.contains_key(&FAMILIA_PESADO))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn arquivo_que_nao_e_fonte_e_recusado() {
        assert!(!fonte_valida(b""));
        assert!(!fonte_valida(b"OTTO"));
        assert!(!fonte_valida(b"PK\x03\x04qualquer coisa"));
        assert!(metricas_verticais(b"nada").is_none());
    }

    #[test]
    fn sem_fontes_do_sistema_caem_nas_do_egui() {
        let defs = definicoes_com(FontesDoSistema::default());
        let prop = &defs.families[&FontFamily::Proportional];
        assert!(prop.iter().any(|n| n == "Ubuntu-Light"), "{prop:?}");
        assert_eq!(defs.families[&FontFamily::Monospace][0], "Hack");
        for familia in [&FAMILIA_LEVE, &FAMILIA_MEDIO, &FAMILIA_PESADO, &FAMILIA_ITALICO, &FAMILIA_AJUDA] {
            assert!(!defs.families[&(*familia).clone()].is_empty());
        }
        for nomes in defs.families.values() {
            for n in nomes {
                assert!(defs.font_data.contains_key(n), "{n} sem dados");
            }
        }
    }

    #[test]
    fn familias_mono_so_as_que_existem() {
        let raiz = std::env::temp_dir().join(format!("databox-fontes-{}", std::process::id()));
        let (a, b) = (raiz.join("a"), raiz.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        // na pasta falsa: a Cascadia com o segundo nome, a Lucida, e uma "Courier" que é pasta e não arquivo
        std::fs::write(b.join("CascadiaMono-Regular.ttf"), b"x").unwrap();
        std::fs::write(a.join("lucon.ttf"), b"x").unwrap();
        std::fs::create_dir_all(a.join("cour.ttf")).unwrap();
        let achadas = familias_mono_em(&[a.clone(), b.clone()]);
        let _ = std::fs::remove_dir_all(&raiz);
        assert_eq!(
            achadas,
            [
                FamiliaMono { nome: "Cascadia Mono", arquivo: b.join("CascadiaMono-Regular.ttf") },
                FamiliaMono { nome: "Lucida Console", arquivo: a.join("lucon.ttf") },
            ]
        );
        assert!(familias_mono_em(&[]).is_empty());
        // o arquivo que não é fonte não é lido como fonte
        assert_eq!(ler_familia_mono(&achadas[0]), None);
    }

    #[test]
    fn trocar_mono_poe_a_familia_a_frente_e_restaurar_tira() {
        let ctx = egui::Context::default();
        let hack = FontDefinitions::default().font_data["Hack"].font.to_vec();
        assert!(fonte_valida(&hack));
        let cadeia = |ctx: &egui::Context| ctx.fonts(|f| f.definitions().families[&FontFamily::Monospace].clone());
        let quadro = |ctx: &egui::Context| ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
        quadro(&ctx);
        let antes = cadeia(&ctx);
        assert!(!trocar_mono(&ctx, b"nada".to_vec(), "Falsa"));
        assert_eq!(mono_escolhida(&ctx), None);
        assert!(trocar_mono(&ctx, hack.clone(), "Cascadia Mono"));
        assert_eq!(mono_escolhida(&ctx).as_deref(), Some("Cascadia Mono"));
        quadro(&ctx);
        let depois = cadeia(&ctx);
        assert_eq!(depois[0], NOME_MONO_ESCOLHIDA);
        assert_eq!(depois[1..], antes[..], "a padrão e as do egui ficam de reserva");
        // trocar de novo não empilha
        assert!(trocar_mono(&ctx, hack, "Courier New"));
        quadro(&ctx);
        assert_eq!(cadeia(&ctx).iter().filter(|n| *n == NOME_MONO_ESCOLHIDA).count(), 1);
        restaurar_mono(&ctx);
        quadro(&ctx);
        assert_eq!(cadeia(&ctx), antes);
        assert_eq!(mono_escolhida(&ctx), None);
    }

    #[cfg(windows)]
    #[test]
    fn segoe_ui_do_windows_entra_em_todos_os_pesos() {
        let sistema = ler_fontes_do_sistema();
        for (nome, b) in [
            ("leve", &sistema.leve),
            ("normal", &sistema.normal),
            ("medio", &sistema.medio),
            ("pesado", &sistema.pesado),
            ("italico", &sistema.italico),
            ("mono", &sistema.mono),
        ] {
            assert!(b.as_ref().is_some_and(|b| fonte_valida(b)), "{nome} ausente");
        }
        let defs = definicoes_com(sistema);
        assert_eq!(defs.families[&FontFamily::Proportional][0], NOME_NORMAL);
        assert_eq!(defs.families[&FAMILIA_MEDIO.clone()][0], NOME_MEDIO);
        assert_eq!(defs.families[&FontFamily::Monospace][0], NOME_MONO);
    }
}
