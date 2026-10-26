"""Peças comuns das pranchetas de cenas da 0.3 (`grafico.py`, `ssh.py`), tiradas das pranchetas que já existem.

A folha de estilo, as definições e o script saem da prancheta 17 (`Planilha.dc.html`), que já traz os das pranchetas 15
e 16 e as regras de formulário; os botões e os ícones saem das outras pranchetas pelo `aria-label` ou pelo texto, e os
sólidos da biblioteca (prancheta 9) pela forma (`g_barras`, `st_info`…). Nada aqui grava: quem grava é o gerador, com
`gravar`, que recusa um arquivo sem a marca dele e um lugar ocupado no canvas.
Só biblioteca padrão; importado pelos geradores desta pasta."""
import json
import re
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
PASTA = AQUI.parent / "canvas"
UM_SVG = r'<svg\b[^>]*>(?:(?!</svg>).)*</svg>'
sys.stdout.reconfigure(encoding="utf-8")

_cache = {}


def ler(nome):
    if nome not in _cache:
        _cache[nome] = (PASTA / f"{nome}.dc.html").read_text(encoding="utf-8")
    return _cache[nome]


def achar(padrao, texto, onde, grupo=1):
    m = re.search(padrao, texto, re.S)
    if not m:
        raise SystemExit(f"pecas: {onde}: não achei {padrao[:90]!r}")
    return m.group(grupo)


def lado(svg, n):
    return re.sub(r'width="[^"]+" height="[^"]+"', f'width="{n}" height="{n}"', svg, count=1)


# --- a folha, as definições e o script ------------------------------------------------------------------------------
def cabeca(titulo, marca, proprio):
    """O `<head>` e o `<helmet>` da prancheta 17, com outro título, sem a marca dela e com as regras desta."""
    base = ler("Planilha")
    c = base[: base.index("</helmet>")]
    c = re.sub(r"<title>.*?</title>", f"<title>{titulo}</title>", c, count=1)
    c, n = re.subn(r"/\* prancheta 17:[^*]*\*/\n", "", c, count=1)
    if n != 1:
        raise SystemExit("pecas: não achei a marca da prancheta 17 na folha")
    return c.replace("</style>", marca + "\n" + segmentado() + proprio + "</style>", 1)


def segmentado():
    """As regras do seletor segmentado da revisão de 19/10/2026, que a folha da prancheta 17 não traz (as do Dados)."""
    css = ler("Dados")
    css = css[css.index("<style>"):css.index("</style>")]
    i = css.index("/* revisão de UI de 19/10/2026 (pranchetas 18 a 24")
    regras = re.findall(r"(\.seg(?: button(?::hover|\.on)?)?)\{([^{}]*)\}", css[i:])
    if [r[0] for r in regras][:4] != [".seg", ".seg button", ".seg button:hover", ".seg button.on"]:
        raise SystemExit("pecas: as regras do segmentado mudaram no Dados.dc.html")
    return "".join(f"{a}{{{b}}}\n" for a, b in regras[:4])


def definicoes(extra_ids=()):
    """As `<defs>` da prancheta 17 e, da biblioteca, os degradês que ela não tem (`extra_ids`)."""
    d = achar(r'(<svg width="0" height="0"[^>]*><defs>.*?</defs></svg>)', ler("Planilha"), "Planilha")
    biblioteca = ler("Icones")
    novos = ""
    for i in extra_ids:
        if f'id="{i}"' in d:
            continue
        novos += achar(r'(<(linearGradient|radialGradient|filter|clipPath)\b[^>]*id="' + re.escape(i) + r'".*?</\2>)',
                       biblioteca, f"Icones ({i})")
    return d.replace("</defs></svg>", novos + "</defs></svg>", 1)


def script(altura):
    base = ler("Planilha")
    s = base[base.index('<script type="text/x-dc"'):]
    s, n = re.subn(r'"\$preview":\{"width":1440,"height":\d+\}', f'"$preview":{{"width":1440,"height":{altura}}}', s)
    if n != 1:
        raise SystemExit("pecas: o script da prancheta 17 não tem o $preview esperado")
    return s


def pagina(titulo, marca, proprio, altura, quadro, extra_ids=()):
    corpo = (f'<div class="est {{{{classes}}}}" style="width:100%;min-height:{altura}px;position:relative;background:var(--ambiente)">\n'
             + definicoes(extra_ids) + "\n" + quadro + "\n</div>")
    return cabeca(titulo, marca, proprio) + "</helmet>\n" + corpo + "\n</x-dc>\n" + script(altura)


# --- botões e ícones --------------------------------------------------------------------------------------------------
ONDE = ("Consulta", "Dados", "Csv", "Estados", "Colunas", "Json")


def botao(aria, de=ONDE, classe=None, dica=None, desligado=False):
    """O botão de ícone (`.ib`) com este `aria-label`, da primeira prancheta que o tem."""
    for nome in de:
        m = re.search(r'(<button class="ib[^"]*"[^>]*aria-label="' + re.escape(aria) + r'"[^>]*>' + UM_SVG + '</button>)',
                      ler(nome), re.S)
        if m:
            b = m.group(1)
            b = re.sub(r'\s(disabled|disabled="")(?=[\s>])', "", b)
            if classe is not None:
                b = re.sub(r'class="ib[^"]*"', f'class="{classe}"', b, count=1)
            if dica is not None:
                b = re.sub(r'title="[^"]*"', f'title="{dica}"', b, count=1)
            if desligado:
                b = b.replace("<button ", '<button disabled="" ', 1)
            return b
    raise SystemExit(f"pecas: nenhum botão {aria!r} em {', '.join(de)}")


def sil(aria, de=ONDE):
    """Só a silhueta do botão."""
    return achar(r'>(' + UM_SVG + ')</button>', botao(aria, de), aria)


def forma(chave, n):
    """Um sólido da biblioteca (prancheta 9) pela forma do `sc-if` (`g_barras`, `st_info`, `a_maquina`…), em n px."""
    svg = achar(r'<sc-if value="\{\{z\.' + re.escape(chave) + r'\}\}"[^>]*>(' + UM_SVG + ')</sc-if>', ler("Icones"), f"Icones ({chave})")
    svg = svg.replace('width="{{z.s}}" height="{{z.s}}"', f'width="{n}" height="{n}"')
    if "{{" in svg:
        raise SystemExit(f"pecas: a forma {chave} da biblioteca depende do molde ({svg[svg.index('{{'):][:40]})")
    return svg


def icone_app(nome, n=16):
    """Um sólido de `design/icones/<nome>.svg` (o desenho de até 24 px que o app usa), sem as `<defs>`: as da página têm
    os mesmos degradês."""
    s = (AQUI.parent / "icones" / f"{nome}.svg").read_text(encoding="utf-8")
    s = re.sub(r"<defs>.*?</defs>", "", s, count=1, flags=re.S)
    s = re.sub(r'^<svg[^>]*>', f'<svg viewBox="0 0 48 48" width="{n}" height="{n}" aria-hidden="true">', s.strip(), count=1)
    return s


def sk(svg, n=16, rot="", classe=""):
    r = f'<span class="rot">{rot}</span>' if rot else ""
    return f'<span class="sk s{n}{" " + classe if classe else ""}" aria-hidden="true">{svg}{r}</span>'


consulta, dados, csv, json_, estados, janelas, claro, conexao = (
    ler(n) for n in ("Consulta", "Dados", "Csv", "Json", "Estados", "Janelas", "DadosClaro", "FuturoConexao"))

FOLHA_CSV = achar(r'<a href="#" role="tab" aria-selected="true"><span class="sk s16 arq" aria-hidden="true">(' + UM_SVG + ')<span class="rot">CSV</span>', csv, "Csv")
FOLHA_SQL = achar(r'<span class="sk s16 arq" aria-hidden="true">(' + UM_SVG + r')<span class="rot">SQL</span></span>Consulta 1', consulta, "Consulta")
FOLHA_L4 = {e: achar(r'<span class="sk s16 arq l4" aria-hidden="true">(' + UM_SVG + r')<span class="rot">' + e + '</span>', conexao, f"FuturoConexao ({e})")
            for e in ("XLSX", "JSON")}
FOLHA_TXT = achar(r'<span class="sk s16 arq" aria-hidden="true">(' + UM_SVG + r')<span class="rot">TXT</span>', conexao, "FuturoConexao (TXT)")
PASTA16 = achar(r'<div class="no" style="padding-left:8px" title="C:\\dados">' + UM_SVG + r'<span class="sk s16" aria-hidden="true">(' + UM_SVG + ')</span>', conexao, "FuturoConexao (pasta)")
BANCO16 = achar(r'<span class="sk s16" aria-hidden="true">(' + UM_SVG + r')</span>exemplo\.db</a>', consulta, "Consulta (banco)")
TABELA16 = achar(r'<span class="sk s16" aria-hidden="true">(' + UM_SVG + r')</span>clientes</a>', consulta, "Consulta (tabela)")
SETA_ABERTA = achar(r'<div class="no" style="padding-left:8px" title="C:\\dados">(' + UM_SVG + ')', conexao, "FuturoConexao (seta)")
SETA_FECHADA = SETA_ABERTA.replace('d="M6 9l6 6 6-6"', 'd="M9 6l6 6-6 6"')
SETA_CAMPO = achar(r'<button class="campo" aria-label="Separador">Ponto e vírgula<span style="flex:1"></span>(' + UM_SVG + ')</button>', csv, "Csv")
FECHAR_X = achar(r'<button class="x" aria-label="Fechar aba">(' + UM_SVG + ')</button>', csv, "Csv")
FECHAR_DLG = achar(r'<button class="ib" aria-label="Fechar" title="Fechar \(Esc\)">(' + UM_SVG + ')</button>', janelas, "Janelas")
OK = achar(r'<div role="status" class="flut"[^>]*>\s*<span class="sk s16" aria-hidden="true">(' + UM_SVG + ')</span>', json_, "Json")
ATENCAO = achar(r'<div class="aviso" role="alert">\s*<span class="sk s24" aria-hidden="true">(' + UM_SVG + ')</span>', claro, "DadosClaro")
ERRO = achar(r'<div class="erro">\s*<span class="sk s24" aria-hidden="true">(' + UM_SVG + ')</span>', consulta, "Consulta")
GIRA = achar(r'(<svg class="ic gira" viewBox="0 0 24 24">.*?</svg>)', estados, "Estados")
COPIAR = achar(r'<button class="btn sutil lista">(' + UM_SVG + ')Copiar caminho</button>', csv, "Csv")
CHECK = '<svg class="ic ck" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5"></path></svg>'


def pastilha(coluna):
    """A pastilha de tipo do cabeçalho do CSV (data, regiao, quantidade, valor)."""
    return achar(r'title="[^"]+">(<span class="sk s16 tipo g\d" aria-hidden="true">' + UM_SVG + r'<span class="rot">[^<]+</span></span>)' + coluna + '</th>', csv, "Csv")


P_DATA, P_TXT, P_INT, P_DEC = pastilha("data"), pastilha("regiao"), pastilha("quantidade"), pastilha("valor")


def folha(ext, n=16):
    if ext in FOLHA_L4:
        return f'<span class="sk s{n} arq l4" aria-hidden="true">{lado(FOLHA_L4[ext], n)}<span class="rot">{ext}</span></span>'
    svg = {"CSV": FOLHA_CSV, "SQL": FOLHA_SQL, "TXT": FOLHA_TXT}[ext]
    if n == 16:
        return f'<span class="sk s16 arq" aria-hidden="true">{svg}<span class="rot">{ext}</span></span>'
    return (f'<span class="sk s{n} arq" aria-hidden="true">{lado(svg, n)}'
            f'<span class="rot" style="font-size:{round(5.5 * n / 16, 1)}px;letter-spacing:0">{ext}</span></span>')


# --- peças de página ------------------------------------------------------------------------------------------------
def estado(titulo, nota, palco, largo=False, estilo_palco=""):
    classe = "estado largo" if largo else "estado"
    st = f' style="{estilo_palco}"' if estilo_palco else ""
    return (f'<section class="{classe}" aria-label="{titulo}"><h2>{titulo}</h2><p class="nota">{nota}</p>'
            f'<div class="palco"{st}>{palco}</div></section>')


def campo(texto, aria, icone="", classe="campo", estilo=""):
    st = f' style="{estilo}"' if estilo else ""
    return (f'<button class="{classe}" aria-haspopup="listbox" aria-label="{aria}"{st}>{icone}<span class="cv">{texto}</span>'
            f'<span style="flex:1"></span>{SETA_CAMPO}</button>')


def mi(texto, atalho="", icone=None, classe="mi", desligado=False):
    ic = icone if icone else '<span class="vz"></span>'
    at = f'<span class="at">{atalho}</span>' if atalho else ""
    dis = ' aria-disabled="true"' if desligado else ""
    return f'<a class="{classe}" role="menuitem" href="#"{dis}>{ic}{texto}{at}</a>'


SEP = '<div class="msep"></div>'


def barra(*itens):
    return '<div role="toolbar" aria-label="Ferramentas (trecho)" class="vidro barra-mini">' + "".join(itens) + "</div>"


SEPARADOR = '<span class="sep"></span>'


def aba(icone, nome, on=False, dica="", sub=""):
    sel = "true" if on else "false"
    t = f' title="{dica}"' if dica else ""
    s = f'<span class="sub">{sub}</span>' if sub else ""
    return (f'<div class="aba{" on" if on else ""}" role="presentation"><a href="#" role="tab" aria-selected="{sel}"{t}>{icone}{nome}{s}</a>'
            f'<button class="x" aria-label="Fechar aba">{FECHAR_X}</button></div>')


def abas(*itens):
    return ('<div role="tablist" aria-label="Abas abertas" style="height:35px;flex:none;display:flex;align-items:flex-end;gap:2px;'
            'padding:0 10px;margin-bottom:-1px;position:relative;z-index:1">' + "".join(itens) + "</div>")


def toast(texto, icone=None, acao=""):
    ic = sk(icone or OK)
    a = f'<button class="btn pq">{acao}</button>' if acao else ""
    cls = "flut toast com-acao" if acao else "flut toast"
    return f'<div role="status" class="{cls}">{ic}<span>{texto}</span>{a}</div>'


def aviso(texto, icone=None, perigo=False, acoes=""):
    ic = icone or (ERRO if perigo else ATENCAO)
    corpo = f'<div class="corpo"><span>{texto}</span><div class="acoes">{acoes}</div></div>' if acoes else f"<span>{texto}</span>"
    return f'<div class="aviso-j{" perigo" if perigo else ""}" role="alert">{sk(ic, 24)}{corpo}</div>'


def numero(v, casas=0):
    """No formato do português: milhar com ponto, decimal com vírgula."""
    t = f"{v:,.{casas}f}"
    return t.replace(",", "X").replace(".", ",").replace("X", ".")


# --- gravar ---------------------------------------------------------------------------------------------------------
def gravar(arquivo, texto, marca, entrada, aceitar_sem_marca=False):
    """Grava a prancheta e a entrada dela no canvas.json (guarda a página e o lugar que já houver). Recusa um arquivo que
    exista sem a marca (a menos de `aceitar_sem_marca`, para a prancheta escrita à mão que o gerador passa a refazer) e
    um lugar que cubra outra prancheta da mesma página."""
    destino = PASTA / arquivo
    if destino.is_file() and marca not in destino.read_text(encoding="utf-8") and not aceitar_sem_marca:
        raise SystemExit(f"{arquivo} existe sem a marca do gerador; não gravei nada")
    lay = PASTA / "canvas.json"
    j = json.loads(lay.read_text(encoding="utf-8"))
    velho = j["boards"].get(arquivo, {})
    novo = dict(entrada)
    for k in ("x", "y", "page"):
        if k in velho:
            novo[k] = velho[k]
    for nome, b in j["boards"].items():
        if nome == arquivo or b.get("page") != novo.get("page"):
            continue
        if b["x"] < novo["x"] + novo["w"] and novo["x"] < b["x"] + b["w"] and b["y"] < novo["y"] + novo["h"] and novo["y"] < b["y"] + b["h"]:
            raise SystemExit(f"o lugar de {arquivo} cobre {nome}; não gravei nada")
    if not destino.is_file() or destino.read_text(encoding="utf-8") != texto:
        destino.write_text(texto, encoding="utf-8", newline="\n")
        print("gravado:", arquivo, len(texto), "bytes")
    else:
        print("igual:", arquivo)
    if velho != novo or arquivo not in j["order"]:
        j["boards"][arquivo] = novo
        if arquivo not in j["order"]:
            j["order"].append(arquivo)
        lay.write_text(json.dumps(j, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
        print(f"canvas.json: {arquivo} em x {novo['x']}, y {novo['y']}, {novo['w']} × {novo['h']}")


def conferir_refs(texto, arquivo):
    faltam = [r for r in sorted(set(re.findall(r'url\(#([A-Za-z0-9_-]+)\)', texto))) if f'id="{r}"' not in texto]
    if faltam:
        raise SystemExit(f"{arquivo}: url(#…) sem definição: {', '.join(faltam)}")
    if re.search(r"\{\{(?!classes\}\})", texto):
        raise SystemExit(f"{arquivo}: sobrou molde {{{{…}}}} fora do classes")
