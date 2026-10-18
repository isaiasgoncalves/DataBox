"""Prancheta 17: a planilha do Excel, só leitura (escuro).
Seis cenas, cada uma com título e legenda, como nas pranchetas 15 e 16, todas com o relatorio_q3.xlsx da prancheta 1
(planilhas Resumo, Vendas, Regiões e uma oculta, Custos):
- (a) a planilha aberta: a barra com só o Atualizar ligado, a aba do documento, a faixa de contexto (a folha XLSX, a
  migalha, o chip Somente leitura e Abrir no programa padrão), a grade com letras de coluna, o número da linha do Excel,
  o título mesclado A1:F1, a linha 1 e a coluna A congeladas com o fio de emenda, números e datas à direita, o erro
  #DIV/0! em --perigo, o booleano, a linha que o filtro esconde esmaecida, a célula ativa, a faixa das planilhas, o
  rodapé, o status e as Propriedades (Arquivo, Planilha, Pasta de trabalho com a caixa das ocultas e Célula);
- (b) muitas planilhas: a faixa com ‹ › e a lista aberta, com a ativa marcada e as ocultas em --t3 e itálico;
- (c) a planilha grande lendo: as linhas ainda sem página a 55%, a rolagem curta e a barra "Lendo a planilha… 43%"
  com Cancelar no rodapé;
- (d) os estados de erro na ficha (senha, formato antigo, arquivo estragado) e a tela sem planilha à vista;
- (e) o Trazer colunas com a origem numa planilha ("Primeira linha é o cabeçalho" ao lado do seletor);
- (f) o menu do cabeçalho (Copiar coluna, Trazer colunas desta planilha), com e sem CSV aberto.
Parte da folha de estilo, das definições e do script da prancheta 16 (Colunas.dc.html), que já traz as das
pranchetas 15 e das janelas, e dos ícones e botões das outras pranchetas, pelo aria-label ou pelo texto, como o
colunas.py. Nenhum ícone novo: tudo vem da biblioteca (prancheta 9). Refaz a prancheta inteira; recusa gravar por cima
de um Planilha.dc.html que não tenha a marca dele e um lugar no canvas.json ocupado por outra prancheta.
Uso: python -I -X utf8 planilha.py [pasta das pranchetas]      (com --conferir, só confere o que já foi gravado)"""
import json
import re
import sys
from datetime import date, timedelta
from pathlib import Path

AQUI = Path(__file__).resolve().parent
ARGS = [a for a in sys.argv[1:] if not a.startswith("--")]
CONFERIR = "--conferir" in sys.argv
PASTA = Path(ARGS[0]) if ARGS else AQUI.parent / "canvas"
sys.stdout.reconfigure(encoding="utf-8")
UM_SVG = r'<svg\b[^>]*>(?:(?!</svg>).)*</svg>'
MARCA = "/* prancheta 17: planilha do Excel (planilha.py) */"
ARQUIVO = "Planilha.dc.html"
ALTURA = 3444
LUGAR = {"x": 1560, "y": 8300, "w": 1440}
TITULO = "17 · Planilha do Excel, só leitura (escuro)"
CENAS = 6


def ler(nome):
    return (PASTA / f"{nome}.dc.html").read_text(encoding="utf-8")


def achar(padrao, texto, onde, grupo=1):
    m = re.search(padrao, texto, re.S)
    if not m:
        raise SystemExit(f"planilha: {onde}: não achei {padrao[:90]!r}")
    return m.group(grupo)


def entrada_canvas():
    return {"expand": "fill", "h": ALTURA, "title": TITULO, "w": LUGAR["w"], "x": LUGAR["x"], "y": LUGAR["y"]}


def cobre(b, alt):
    return (b["x"] < LUGAR["x"] + LUGAR["w"] and LUGAR["x"] < b["x"] + b["w"]
            and b["y"] < LUGAR["y"] + alt and LUGAR["y"] < b["y"] + b["h"])


# --- a conferência ------------------------------------------------------------------------------------------------
if CONFERIR:
    erros = []
    destino = PASTA / ARQUIVO
    if not destino.is_file():
        raise SystemExit(f"planilha: {ARQUIVO} não existe; rode o gerador antes")
    texto = destino.read_text(encoding="utf-8")
    if MARCA not in texto:
        erros.append("falta a marca do gerador no <style>")
    j = json.loads((PASTA / "canvas.json").read_text(encoding="utf-8"))
    b = j["boards"].get(ARQUIVO)
    if not b:
        erros.append("o canvas.json não tem a prancheta")
    else:
        if (b["x"], b["y"], b["w"]) != (LUGAR["x"], LUGAR["y"], LUGAR["w"]):
            erros.append(f"lugar errado no canvas.json: {b['x']}, {b['y']}, {b['w']}")
        if b["title"] != TITULO:
            erros.append(f"título diferente no canvas.json: {b['title']!r}")
        for nome, o in j["boards"].items():
            if nome != ARQUIVO and cobre(o, b["h"]):
                erros.append(f"o lugar cobre {nome}")
        m = re.search(r'min-height:(\d+)px', texto[texto.index('<div class="est '):])
        if not m or int(m.group(1)) != b["h"]:
            erros.append(f"a altura da página ({m and m.group(1)}) não é a do canvas.json ({b['h']})")
        if '"$preview":{"width":1440,"height":%d}' % b["h"] not in texto:
            erros.append("o $preview do script não tem a altura do canvas.json")
    if ARQUIVO not in j["order"]:
        erros.append("a prancheta não está na order do canvas.json")
    cenas = re.findall(r'<section class="estado[^"]*" aria-label="([^"]+)"><h2>([^<]+)</h2><p class="nota">([^<]{40,})</p>', texto)
    if len(cenas) != CENAS:
        erros.append(f"esperava {CENAS} cenas com título e legenda, achei {len(cenas)}")
    for ref in sorted(set(re.findall(r'url\(#([A-Za-z0-9_-]+)\)', texto))):
        if f'id="{ref}"' not in texto:
            erros.append(f"url(#{ref}) sem definição")
    biblioteca = ler("Icones")
    for coisa, prova in (("a folha XLSX", "XLSX"), ("a silhueta trazer_colunas", '<div class="nome">Trazer colunas</div>'),
                         ("o Bloqueado", "st_bloqueado")):
        if prova not in biblioteca:
            erros.append(f"{coisa} não está na biblioteca (prancheta 9)")
    if "XLSX</span>" not in texto:
        erros.append("a folha XLSX não aparece na prancheta")
    if erros:
        raise SystemExit("planilha --conferir:\n  - " + "\n  - ".join(erros))
    print(f"planilha --conferir: ok ({len(cenas)} cenas)")
    for i, (_, titulo, _) in enumerate(cenas):
        print(f"  {'abcdef'[i]}) {titulo}")
    sys.exit(0)


def lado(svg, n):
    return re.sub(r'width="[^"]+" height="[^"]+"', f'width="{n}" height="{n}"', svg, count=1)


def estilo(texto):
    return texto[texto.index("<style>") + len("<style>"):texto.index("</style>")].strip("\n").split("\n")


colunas_, csv, dados, json_, consulta, claro, icones, janelas = (
    ler(n) for n in ("Colunas", "Csv", "Dados", "Json", "Consulta", "DadosClaro", "Icones", "Janelas"))

# --- folha: a da prancheta 16 (que já tem a do Estados e as regras de formulário) + as desta prancheta ---------------
cabeca = colunas_[: colunas_.index("</helmet>")]
cabeca = re.sub(r"<title>.*?</title>", "<title>Planilha do Excel, só leitura</title>", cabeca, count=1)
cabeca = re.sub(r"/\* prancheta 16:[^*]*\*/\n", "", cabeca, count=1)
if "/* prancheta 16" in cabeca:
    raise SystemExit("planilha: não consegui tirar a marca da prancheta 16 da folha")
PROPRIO = MARCA + """
.det dd.caminho{display:flex;min-width:0;white-space:nowrap;overflow-wrap:normal}
.det dd.caminho .ini{overflow:hidden;text-overflow:ellipsis;min-width:0}
.det dd.caminho .fim{flex:none}
.op{display:flex;align-items:center;gap:8px;color:var(--t)}
.estado .palco{flex:1}
.so-leit{display:inline-flex;align-items:center;gap:6px;height:22px;padding:0 10px 0 6px;border-radius:999px;background:var(--h);border:1px solid var(--vidrob);color:var(--t2);font-size:11px;font-weight:600;white-space:nowrap}
.xlw{flex:1;min-height:0;overflow:hidden;position:relative;background-color:var(--c);background-image:linear-gradient(to bottom,var(--sup-meio),var(--bt2)),linear-gradient(var(--btb),var(--btb));background-size:100% 30px,100% 1px;background-position:0 0,0 29px;background-repeat:no-repeat}
.xl{width:auto;table-layout:fixed;border-collapse:separate;border-spacing:0}
.xl th{padding:0 6px;text-align:center;color:var(--t2);background:transparent;border-bottom-color:transparent}
.xl th.cant{padding:0}
.xl td{padding:0 8px}
.xl td.lin{padding:0 8px;width:56px}
.xl td.d{text-align:right;font-variant-numeric:tabular-nums}
.xl td.c{text-align:center}
.xl td.err{text-align:center;color:var(--perigo)}
.xl td.mesc{text-align:left;border-right:0}
.xl tr.cl td{border-bottom:2px solid var(--b2)}
.xl th.cc,.xl td.cc{border-right:2px solid var(--b2)}
.xl tr.ocul td{color:var(--t3)}
.xl tr.ocul td.lin{font-style:italic}
.xl tr.fant td{opacity:.55}
.xl th.selcol{background:var(--sel);color:var(--t)}
.xl td.selcol{background:var(--tint)}
.xl td.vz{background:transparent}
.folhas{height:32px;flex:none;display:flex;align-items:center;gap:4px;padding:0 8px;border-top:1px solid var(--btb);background:var(--textura),linear-gradient(to bottom,var(--sup-meio),var(--bt2));background-attachment:fixed,scroll}
.fl-rolo{flex:1;min-width:0;overflow:hidden;display:flex;height:24px}
.fl-trilho{display:flex;gap:4px;flex:none}
.fp{height:24px;padding:0 12px;border-radius:999px;border:1px solid transparent;background:transparent;color:var(--t2);font:inherit;white-space:nowrap;cursor:pointer;display:inline-flex;align-items:center}
.fp:hover{background:var(--h);color:var(--t)}
.fp[aria-selected=true]{background:var(--textura),var(--volneu);border-color:var(--btb);color:var(--t);font-weight:600;box-shadow:inset 0 1px 0 var(--btluz),inset 0 -2px 3px rgba(0,0,0,.18),var(--btsomb)}
.fp.oculta{color:var(--t3);font-style:italic}
.fp.sem-fim{opacity:.55}
.menu-ctx.folhas-m{width:236px;max-height:332px;overflow:hidden}
.mi .ck{width:14px;flex:none}
.mi.oc{color:var(--t3);font-style:italic}
.mi.oc .at{font-style:normal}
.rolagem-g{position:absolute;right:3px;width:4px;border-radius:999px;background:var(--fbc);opacity:.8}
.rolagem-m{position:absolute;right:4px;top:8px;width:4px;height:150px;border-radius:999px;background:var(--fbc);opacity:.7}
.prog{display:inline-flex;align-items:center;gap:10px;color:var(--t2)}
.prog .barra{width:160px}
.ficha-a{display:flex;flex-direction:column;align-items:center;justify-content:center;gap:16px;padding:26px 24px;flex:1;min-height:170px;text-align:center}
.ficha-a .acoes{display:flex;gap:8px}
.aviso-j.perigo{background:color-mix(in oklab,var(--perigo) 10%,transparent);border-color:color-mix(in oklab,var(--perigo) 30%,transparent)}
.cartoes{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:14px}
.cartao{display:flex;flex-direction:column;gap:8px;min-width:0}
.cartao>.doc{flex:1}
.mini-prop{width:228px;flex:none;border-radius:10px;display:flex;flex-direction:column;overflow:hidden}
.mini-prop .det{padding-bottom:10px}
.so-menu{position:relative;height:96px}
.doc-rel{position:relative;display:flex;flex-direction:column}
.casar .lado{display:flex;flex-direction:column;gap:6px}
.bloco-tc-meta{margin-left:0}
"""
cabeca = cabeca.replace("</style>", PROPRIO + "</style>", 1)
defs = achar(r'(<svg width="0" height="0"[^>]*><defs>.*?</defs></svg>)', colunas_, "Colunas")

# --- peças tiradas das pranchetas -----------------------------------------------------------------------------------
UMA = r'<button class="ib[^"]*"[^>]*aria-label="'


def botao(texto, aria, onde):
    return achar(r'(' + UMA + re.escape(aria) + r'"[^>]*>' + UM_SVG + '</button>)', texto, onde)


def svg_de(texto, aria, onde):
    return achar(UMA + re.escape(aria) + r'"[^>]*>(' + UM_SVG + ')</button>', texto, onde)


def desligado(b):
    return b.replace('<button class="ib', '<button disabled="" class="ib', 1)


xlsx_svg = achar(r'<span class="sk s16 arq l4" aria-hidden="true">(' + UM_SVG + r')<span class="rot">XLSX</span></span><span>relatorio_q3\.xlsx</span>', csv, "Csv")
doc_csv = achar(r'<a href="#" role="tab" aria-selected="true"><span class="sk s16 arq" aria-hidden="true">(' + UM_SVG + ')<span class="rot">CSV</span>', csv, "Csv")
fechar_x = achar(r'<button class="x" aria-label="Fechar aba">(' + UM_SVG + ')</button>', csv, "Csv")
fechar_dlg = achar(r'<button class="ib" aria-label="Fechar" title="Fechar \(Esc\)">(' + UM_SVG + ')</button>', janelas, "Janelas")
seta_campo = achar(r'<button class="campo" aria-label="Separador">Ponto e vírgula<span style="flex:1"></span>(' + UM_SVG + ')</button>', csv, "Csv")
seta_migalha = achar(r'<span style="margin-left:3px">config\.json</span>(' + UM_SVG + ')conexoes', json_, "Json")
sec_icone = achar(r'<div class="sec">(' + UM_SVG + ')Arquivo</div>', csv, "Csv")
copiar_svg = achar(r'<button class="btn sutil lista">(' + UM_SVG + ')Copiar caminho</button>', csv, "Csv")
ok_svg = achar(r'<div role="status" class="flut"[^>]*>\s*<span class="sk s16" aria-hidden="true">(' + UM_SVG + ')</span>', json_, "Json")
atencao = achar(r'<div class="aviso" role="alert">\s*<span class="sk s24" aria-hidden="true">(' + UM_SVG + ')</span>', claro, "DadosClaro")
erro_svg = achar(r'<div class="erro">\s*<span class="sk s24" aria-hidden="true">(' + UM_SVG + ')</span>', consulta, "Consulta")
pag_ant = svg_de(dados, "Página anterior", "Dados")
pag_prox = svg_de(dados, "Próxima página", "Dados")
TRAZER = achar(r'<button class="ib" aria-label="Trazer colunas" title="Trazer colunas">(' + UM_SVG + ')</button>', colunas_, "Colunas")
CHECK = '<svg class="ic ck" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5"></path></svg>'
CHECK_OP = '<svg class="ic" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5"></path></svg>'


def cadeado(n):
    """O Bloqueado da biblioteca (prancheta 9) num tamanho fixo de até 24 px, com o contorno firme do item 13."""
    svg = achar(r'<sc-if value="\{\{z\.st_bloqueado\}\}"[^>]*>(' + UM_SVG + ')</sc-if>', icones, "Icones")
    svg = svg.replace('width="{{z.s}}" height="{{z.s}}"', f'width="{n}" height="{n}"')
    svg = svg.replace('<rect class="cont" filter="url(#icSombra)"', '<rect filter="url(#icSombra)"')
    return re.sub(r'(<rect filter="url\(#icSombra\)"[^>]*style=")stroke-opacity:\.35"', r'\1stroke-opacity:.35;stroke-width:2.4"', svg, count=1)


def pastilha(coluna):
    return achar(r'title="[^"]+">(<span class="sk s16 tipo g\d" aria-hidden="true">' + UM_SVG + r'<span class="rot">[^<]+</span></span>)' + coluna + '</th>', csv, "Csv")


P_DATA, P_TXT, P_INT, P_DEC = pastilha("data"), pastilha("regiao"), pastilha("quantidade"), pastilha("valor")


def folha(n=16):
    """A folha XLSX (a etiqueta de 4 letras, classe l4)."""
    if n == 16:
        return f'<span class="sk s16 arq l4" aria-hidden="true">{xlsx_svg}<span class="rot">XLSX</span></span>'
    corpo = round(4.9 * n / 16, 1)
    return (f'<span class="sk s{n} arq l4" aria-hidden="true">{lado(xlsx_svg, n)}'
            f'<span class="rot" style="font-size:{corpo}px;letter-spacing:-.2px;text-shadow:0 .5px .5px rgba(0,0,0,.45)">XLSX</span></span>')


def csv16():
    return f'<span class="sk s16 arq" aria-hidden="true">{doc_csv}<span class="rot">CSV</span></span>'


def estado(titulo, nota, palco, largo=False, estilo_palco=""):
    classe = "estado largo" if largo else "estado"
    st = f' style="{estilo_palco}"' if estilo_palco else ""
    return (f'<section class="{classe}" aria-label="{titulo}"><h2>{titulo}</h2><p class="nota">{nota}</p>'
            f'<div class="palco"{st}>{palco}</div></section>')


def campo(texto, aria, classe="campo", icone="", estilo_=""):
    st = f' style="{estilo_}"' if estilo_ else ""
    return (f'<button class="{classe}" aria-haspopup="listbox" aria-label="{aria}"{st}>{icone}<span>{texto}</span>'
            f'<span style="flex:1"></span>{seta_campo}</button>')


def mi(texto, atalho="", icone=None, classe="mi", desligado_=False, extra=""):
    ic = icone if icone else '<span class="ck"></span>'
    at = f'<span class="at">{atalho}</span>' if atalho else ""
    dis = ' aria-disabled="true"' if desligado_ else ""
    return f'<a class="{classe}" role="menuitem" href="#"{dis}>{ic}{texto}{at}{extra}</a>'


SEP = '<div class="msep"></div>'
CHIP_LEITURA = f'<span class="so-leit">{cadeado(16)}Somente leitura</span>'
ABRIR_PADRAO = '<button class="btn sutil pq">Abrir no programa padrão</button>'

# --- os dados: o relatorio_q3.xlsx, planilha Vendas (a mesma da prancheta 1: 1.204 linhas, A1:F1204) ------------------
REG = ["Sul", "Sudeste", "Nordeste", "Norte", "Centro-Oeste"]
PROD = ["Caderno universitário", "Caneta gel 0,7 mm", "Mochila escolar G", "Agenda 2026 diária", "Estojo duplo", "Marca-texto (kit 6)"]
CABECALHO = ["Região", "Produto", "Fechado em", "Receita", "Atingido", "Meta batida"]
LARG_A = [("A", 131), ("B", 215), ("C", 103), ("D", 131), ("E", 103), ("F", 117)]  # largura do arquivo: round(caracteres * 7 + 5)
ERRO_LINHA, OCULTA_LINHA, ATIVA = 12, 9, ("E", 6)


def brl(v):
    return "R$ " + f"{v:,.2f}".replace(",", "X").replace(".", ",").replace("X", ".")


def valores_vendas(n):
    """As seis células da linha n (n >= 3) como o Excel mostra: (texto, classe)."""
    k = n - 2
    pct = 72 + (k * 37) % 61
    data = (date(2026, 7, 1) + timedelta(days=k - 1)).strftime("%d/%m/%Y")
    receita = brl(2200 + (k * 7919) % 11800 + ((k * 37) % 100) / 100)
    if n == ERRO_LINHA:
        return [(REG[k % 5], ""), (PROD[(k * 5) % 6], ""), (data, "d"), (receita, "d"), ("#DIV/0!", "err"), ("FALSO", "c")]
    return [(REG[k % 5], ""), (PROD[(k * 5) % 6], ""), (data, "d"), (receita, "d"), (f"{pct}%", "d"),
            ("VERDADEIRO" if pct >= 100 else "FALSO", "c")]


def th_xl(letra, largura, primeira=False, sel=False):
    cls = " ".join(c for c, on in (("cc", primeira), ("selcol", sel)) if on)
    return f'<th{f" class=\"{cls}\"" if cls else ""} style="width:{largura}px">{letra}</th>'


def tabela(colunas, linhas, selcol=None, congelado=True, ativa=None, ocultas=(), pendentes=()):
    """A grade da planilha: letras de coluna, número da linha, as larguras do arquivo e o painel congelado (linha 1 e
    coluna A). `linhas` é uma lista de (n, [(texto, classe), ...]); a linha 1 pode vir mesclada (um item só, com colspan)."""
    topo = '<th class="cant" style="width:56px"></th>' + "".join(
        th_xl(letra, px, primeira=(i == 0 and congelado), sel=(letra == selcol)) for i, (letra, px) in enumerate(colunas))
    corpo = []
    for n, cels in linhas:
        cl = " ".join(c for c, on in (("cl", congelado and n == 1), ("ocul", n in ocultas), ("fant", n in pendentes)) if on)
        tds = [f'<td class="lin">{n:,}'.replace(",", ".") + "</td>"]
        letras = [c[0] for c in colunas]
        col = 0
        for item in cels:
            texto, classe = item[0], item[1]
            span = item[2] if len(item) > 2 else 1
            letra = letras[col]
            cls = [classe] if classe else []
            if col == 0 and congelado and n != 1:
                cls.append("cc")
            if letra == selcol:
                cls.append("selcol")
            if ativa == (letra, n):
                cls.append("ativa")
            attr = f' colspan="{span}"' if span > 1 else ""
            tds.append(f'<td{f" class=\"{" ".join(cls)}\"" if cls else ""}{attr}>{texto}</td>')
            col += span
        corpo.append(f'<tr{f" class=\"{cl}\"" if cl else ""}>' + "".join(tds) + "</tr>")
    largura = 56 + sum(px for _, px in colunas)
    return (f'<table class="grade xl" style="width:{largura}px"><thead><tr>{topo}</tr></thead><tbody>'
            + "".join(corpo) + "</tbody></table>")


TITULO_A1 = "Vendas do 3º trimestre de 2026 · por região e produto"


def linhas_vendas(de, ate, mesclar=True):
    out = []
    for n in range(de, ate + 1):
        if n == 1:
            out.append((1, [(TITULO_A1, "mesc", 6)] if mesclar else [(TITULO_A1, "")]))
        elif n == 2:
            out.append((2, [(t, "") for t in CABECALHO]))
        else:
            out.append((n, valores_vendas(n)))
    return out


# --- a faixa das planilhas ---------------------------------------------------------------------------------------
def faixa_folhas(itens, ativa, recuo=0, setas=(True, True), lista=False, vazia=False):
    """itens: (nome, estado) com estado '', 'oculta' ou 'muito oculta'; `recuo` é quanto a faixa já rolou para a esquerda."""
    pilulas = []
    total = len(itens)
    for i, (nome, est) in enumerate(itens, 1):
        rotulo = f"{nome}, planilha {i} de {total}" + (f", {est}" if est else "")
        cls = "fp" + (" oculta" if est else "")
        sel = "true" if nome == ativa else "false"
        dica = f"{nome} ({est})" if est else nome
        pilulas.append(f'<button class="{cls}" role="tab" aria-selected="{sel}" aria-label="{rotulo}" title="{dica}">{nome}</button>')
    esq = f'<button class="ib pq" aria-label="Rolar as planilhas para a esquerda" title="Rolar as planilhas para a esquerda"{"" if setas[0] else " disabled=\"\""}>{pag_ant}</button>'
    dir_ = f'<button class="ib pq" aria-label="Rolar as planilhas para a direita" title="Rolar as planilhas para a direita"{"" if setas[1] else " disabled=\"\""}>{pag_prox}</button>'
    fim = ""
    if lista:
        fim = (f'<button class="ib pq on" aria-haspopup="menu" aria-expanded="true" aria-label="Planilhas" title="Planilhas">'
               f'{seta_campo}</button>')
    st = f' style="margin-left:-{recuo}px"' if recuo else ""
    return (f'<div class="folhas" role="tablist" aria-label="Planilhas">{esq}{dir_}'
            f'<div class="fl-rolo"><div class="fl-trilho"{st}>{"".join(pilulas)}</div></div>{fim}</div>')


FOLHAS_A = [("Resumo", ""), ("Vendas", ""), ("Regiões", ""), ("Custos", "oculta")]


def rodape(*partes):
    return f'<div class="rodape">{"".join(partes)}</div>'


def faixa_ctx(nome, migalha, nome_folha=None):
    """A faixa de 42 px: a folha XLSX de 16, o nome, a migalha, o chip e a saída para o programa padrão."""
    seta = seta_migalha
    sub = "".join(f'{seta}<span>{p}</span>' for p in migalha[:-1])
    ultimo = f'{seta}<strong style="color:var(--t)">{migalha[-1]}</strong>' if migalha else ""
    return (f'<div class="faixa"><span class="migalha">{folha(16)}<span style="margin-left:3px;color:var(--t);font-weight:600">{nome}</span>'
            f'{sub}{ultimo}</span><span style="flex:1"></span>{CHIP_LEITURA}{ABRIR_PADRAO}</div>')


# --- (a) a planilha aberta ---------------------------------------------------------------------------------------
JANELA_H = 884
atualizar = botao(consulta, "Atualizar", "Consulta")
atualizar = re.sub(r'title="[^"]*"', 'title="Atualizar relatorio_q3.xlsx (F5)"', atualizar, count=1)
barra_a = ('<div role="toolbar" aria-label="Ferramentas (trecho)" class="vidro barra-mini">'
           + botao(csv, "Abrir pasta", "Csv") + botao(csv, "Abrir arquivo", "Csv") + desligado(botao(csv, "Salvar", "Csv"))
           + '<span class="sep"></span>'
           + desligado(botao(csv, "Nova linha", "Csv")) + desligado(botao(csv, "Nova coluna", "Csv"))
           + desligado(botao(csv, "Duplicar", "Csv")) + desligado(botao(csv, "Excluir", "Csv"))
           + desligado(botao(csv, "Desfazer", "Csv")) + desligado(botao(csv, "Refazer", "Csv"))
           + '<span class="sep"></span>'
           + desligado(botao(csv, "Filtrar", "Csv")) + atualizar
           + '<span class="sep"></span>' + desligado(botao(csv, "Exportar", "Csv")) + "</div>")
abas_a = ('<div role="tablist" aria-label="Abas abertas" style="height:35px;flex:none;display:flex;align-items:flex-end;gap:2px;padding:0 10px;margin-bottom:-1px;position:relative;z-index:1">'
          f'<div class="aba" role="presentation"><a href="#" role="tab" aria-selected="false">{csv16()}vendas_2025.csv</a><button class="x" aria-label="Fechar aba">{fechar_x}</button></div>'
          f'<div class="aba on" role="presentation"><a href="#" role="tab" aria-selected="true">{folha(16)}relatorio_q3.xlsx</a><button class="x" aria-label="Fechar aba">{fechar_x}</button></div>'
          "</div>")
linhas_a = linhas_vendas(1, 28)
grade_a = '<div class="xlw">' + tabela(LARG_A, linhas_a, congelado=True, ativa=ATIVA, ocultas=(OCULTA_LINHA,)) + "</div>"
doc_a = ('<div class="doc" style="flex:1">' + faixa_ctx("relatorio_q3.xlsx", ["Vendas", "E6"]) + grade_a
         + faixa_folhas(FOLHAS_A, "Vendas", setas=(False, False))
         + rodape("<span>1.204 linhas · 6 colunas · A1:F1204</span>", '<span style="color:var(--t3)">· 12 mescladas</span>') + "</div>")


def dl(*pares):
    return '<dl class="det">' + "".join(f"<dt>{k}</dt><dd{c}>{v}</dd>" for k, v, c in
                                          ((p[0], p[1], p[2] if len(p) > 2 else "") for p in pares)) + "</dl>"


def sec(titulo, sub=""):
    s = f'<span style="font-weight:400;color:var(--t2)">{sub}</span>' if sub else ""
    return f'<div class="sec">{sec_icone}{titulo}{s}</div>'


def botao_lista(texto, svg):
    return f'<div style="padding:0 8px 10px"><button class="btn sutil lista">{svg}{texto}</button></div>'


def caixa(texto, marcada):
    ck = ' checked=""' if marcada else ""
    return f'<label class="op" style="padding:0 14px 14px"><input type="checkbox"{ck}>{texto}</label>'


NOTA_FILTRO = '<span style="color:var(--t3)">(as linhas que ele esconde aparecem esmaecidas)</span>'
prop_a = (
    f'<aside class="vidro" aria-label="Propriedades" style="width:272px;flex:none;border-radius:10px;display:flex;flex-direction:column;overflow:hidden">'
    '<div class="cab"><span class="tit">Propriedades</span></div><div style="flex:1;min-height:0;overflow:hidden">'
    + sec("Arquivo")
    + dl(("Caminho", '<span class="ini">C:\\relatorios\\</span><span class="fim">relatorio_q3.xlsx</span>', ' class="mono caminho" title="C:\\relatorios\\relatorio_q3.xlsx"'),
         ("Tamanho", "184 KB"), ("Modificado em", "12/10/2026 16:20"))
    + botao_lista("Copiar caminho", copiar_svg)
    + sec("Planilha", "Vendas")
    + dl(("Nome", "Vendas"), ("Estado", "visível"), ("Dimensão", "A1:F1204"), ("Linhas", "1.204"), ("Colunas", "6"),
         ("Mescladas", "12"), ("Congelado", "1 linha e 1 coluna"),
         ("Filtro do Excel", f"A2:F1204<br>{NOTA_FILTRO}"), ("Colunas ocultas", '<span style="color:var(--t3)">nenhuma</span>'))
    + sec("Pasta de trabalho")
    + dl(("Planilhas", "4 · 1 oculta"), ("Outras folhas", "1 gráfico (não aparece)"), ("Sistema de datas", "1900"))
    + caixa("Mostrar planilhas ocultas", True)
    + sec("Célula", "E6")
    + dl(("Referência", "E6"), ("Tipo", "número"), ("Valor", "0,98"), ("Formato", "Percentual"),
         ("Fórmula", "= D6 / Resumo!C3", ' class="mono"'))
    + botao_lista("Copiar a fórmula", copiar_svg)
    + '<p style="margin:0;padding:0 14px 14px;color:var(--t3);font-size:11px;line-height:16px">Fórmulas não são recalculadas; o valor é o último que o Excel calculou.</p>'
    + "</div></aside>")
status_a = ('<footer class="vidro" style="height:26px;flex:none;border-radius:10px;display:flex;align-items:center;gap:14px;padding:0 14px;font-size:11px;color:var(--t2)">'
            '<span style="display:inline-flex;align-items:center;gap:6px"><span aria-hidden="true" style="width:7px;height:7px;border-radius:50%;background:var(--t3)"></span>relatorio_q3.xlsx</span>'
            '<span>Somente leitura</span><span style="flex:1"></span><span>XLSX · Vendas</span></footer>')
dica_a = ('<div role="tooltip" class="flut dica-col" style="left:874px;top:176px">'
          'Painel congelado: a linha 1 e a coluna A ficam no lugar quando a planilha rola. O fio de 2 px marca a emenda.</div>')
cena_a = estado(
    "A planilha aberta",
    "Abre só para leitura, na primeira planilha visível (a ativa do arquivo, se estiver à vista). Em cima, a faixa de contexto "
    "com a folha XLSX, o nome, a migalha Vendas › E6 (a mesclada mostra o intervalo, A1:F1), o chip Somente leitura e a "
    "saída para o programa padrão; na barra só o Atualizar (F5) fica ligado. A grade tem as letras de coluna, o número "
    "da linha do Excel e as larguras do arquivo (caracteres × 7 + 5 px); texto à esquerda, números e datas à direita, "
    "booleano e erro no centro, o erro em --perigo. O título A1:F1 é uma célula mesclada, a linha 1 e a coluna A estão "
    "congeladas (fio de 2 px na emenda) e a linha 9, que o filtro do Excel esconde, aparece esmaecida. As planilhas ficam "
    "numa faixa própria de 32 px entre a grade e o rodapé; Custos é oculta e só aparece com a caixa Mostrar planilhas "
    "ocultas, que fica nas Propriedades. As seções Arquivo, Planilha, Pasta de trabalho e Célula vêm das decisões 19 e "
    "22; o texto da fórmula só existe no arquivo.",
    barra_a
    + f'<div style="display:flex;gap:8px;height:{JANELA_H}px;flex:none"><section style="flex:1;min-width:0;display:flex;flex-direction:column">'
    + abas_a + doc_a + "</section>" + prop_a + "</div>" + status_a + dica_a,
    largo=True)

# --- (b) muitas planilhas ------------------------------------------------------------------------------------------
FOLHAS_B = [("Resumo", ""), ("Vendas", ""), ("Regiões", ""), ("Metas", ""), ("Custos", "oculta"), ("Comissões", ""), ("Estoque", ""),
            ("Devoluções", ""), ("Parâmetros", "muito oculta"), ("Câmbio", ""), ("Calendário fiscal", ""), ("Notas e fontes", "")]
LARG_B = [("A", 96), ("B", 170), ("C", 98), ("D", 118), ("E", 84), ("F", 100)]
itens_menu = ""
for nome, est in FOLHAS_B:
    if nome == "Vendas":
        itens_menu += mi(nome, icone=CHECK, classe="mi on")
    elif est:
        itens_menu += mi(nome, classe="mi oc", extra=f'<span class="at">{est}</span>')
    else:
        itens_menu += mi(nome)
menu_b = ('<div role="menu" aria-label="Planilhas" class="flut menu-ctx folhas-m" style="right:8px;bottom:70px">' + itens_menu
          + '<span class="rolagem-m" aria-hidden="true"></span></div>')
doc_b = ('<div class="doc-rel"><div class="doc">' + faixa_ctx("relatorio_q3.xlsx", ["Vendas", "A3"]).replace(ABRIR_PADRAO, "").replace(CHIP_LEITURA, "")
         + '<div class="xlw" style="height:310px;flex:none">' + tabela(LARG_B, linhas_vendas(1, 10), ativa=("A", 3)) + "</div>"
         + faixa_folhas(FOLHAS_B, "Vendas", recuo=30, lista=True)
         + rodape("<span>1.204 linhas · 6 colunas · A1:F1204</span>", '<span style="color:var(--t3)">· 12 mescladas</span>')
         + "</div>" + menu_b + "</div>")
cena_b = estado(
    "Muitas planilhas",
    "Quando as abas não cabem, a faixa ganha ‹ › nas pontas (a roda do mouse também rola; os botões desligam no fim) e, "
    "à direita, o botão da lista, que só existe nesse caso. A lista é o menu de sempre: a ativa marcada com ✓ e as ocultas "
    "em --t3 e itálico, com a etiqueta \"oculta\" ou \"muito oculta\" (as duas só aparecem com a caixa das Propriedades "
    "ligada). Ctrl+PageDown e Ctrl+PageUp trocam de planilha de dentro da grade; ←/→ andam entre as abas quando a faixa "
    "tem o foco. Nome longo corta com \"…\" a 22 caracteres e a dica diz o nome inteiro.",
    doc_b, estilo_palco="padding:14px 14px 14px")

# --- (c) a planilha grande lendo --------------------------------------------------------------------------------------
LARG_C = [("A", 82), ("B", 90), ("C", 214), ("D", 96), ("E", 108), ("F", 118)]
HISTORICO = ["PIX RECEBIDO LOJA CENTRO", "TARIFA MANUTENCAO CONTA", "PAGAMENTO FORNECEDOR 0418", "TED ENVIADA FOLHA", "DEPOSITO DINHEIRO AG 0231",
             "DEBITO AUTOMATICO ENERGIA", "COMPRA CARTAO MAQUININHA", "RESGATE CDB AUTOMATICO"]


def linha_mov(n):
    k = n - 4219
    valor = [1250.00, -89.90, -4310.55, -18422.00, 3000.00, -612.37, 842.10, 5000.00][k % 8]
    saldo = 48912.37 + sum([1250.00, -89.90, -4310.55, -18422.00, 3000.00, -612.37, 842.10, 5000.00][i % 8] for i in range(k))
    br = lambda v: f"{v:,.2f}".replace(",", "X").replace(".", ",").replace("X", ".")
    return [("04/01/2025", "d"), ("001-4", ""), (HISTORICO[k % 8], ""), (f"DOC {880412 + k}", ""), (br(valor), "d"), (br(saldo), "d")]


carregadas = list(range(4219, 4225))
pendentes_l = list(range(4225, 4229))
linhas_c = [(n, linha_mov(n)) for n in carregadas] + [(n, [("", "")] * 6) for n in pendentes_l]
prog = ('<span class="prog" aria-live="polite"><span>Lendo a planilha… 43%</span>'
        '<div class="barra" role="progressbar" aria-label="Lendo a planilha" aria-valuemin="0" aria-valuemax="100" aria-valuenow="43"><div style="width:43%"></div></div>'
        '<button class="btn sutil pq">Cancelar</button></span>')
FOLHAS_C = [("Lançamentos", ""), ("Contas", ""), ("Plano de contas", "")]
grade_c = ('<div class="xlw" style="height:310px;flex:none">' + tabela(LARG_C, linhas_c, congelado=False, ativa=("C", 4222), pendentes=pendentes_l)
           + '<span class="rolagem-g" aria-hidden="true" style="top:46px;height:30px"></span></div>')
doc_c = ('<div class="doc">' + faixa_ctx("movimentos_2025.xlsx", ["Lançamentos", "C4222"]).replace(ABRIR_PADRAO, "") + grade_c
         + faixa_folhas(FOLHAS_C, "Lançamentos", setas=(False, False))
         + rodape("<span>412.388 linhas · 9 colunas · A1:I412388</span>", '<span style="flex:1"></span>' + prog) + "</div>")
cena_c = estado(
    "Planilha grande lendo",
    "A primeira página (128 linhas) aparece em milissegundos e o índice corre em segundo plano. Quem rola para longe antes "
    "de ele acabar vê o número da linha e as células em branco a 55% enquanto o bloco chega (aqui, da linha 4.225 em "
    "diante; os blocos são de 128 linhas alinhados ao Excel). O rodapé mostra a dimensão declarada, a barra \"Lendo a "
    "planilha… 43%\" e o Cancelar, só depois de 400 ms de espera, e a barra de rolagem cresce com o índice. Ao terminar, "
    "as mesclas e o filtro entram numa repintura só. O Cancelar deixa a planilha navegável com o que já foi visto.",
    doc_c)

# --- (d) os erros e a tela sem planilha à vista -----------------------------------------------------------------------------
def aviso(texto, perigo=False):
    ic = erro_svg if perigo else atencao
    return (f'<div class="aviso-j{" perigo" if perigo else ""}" role="alert"><span class="sk s24" aria-hidden="true">{ic}</span>'
            f"<span>{texto}</span></div>")


def faixa_ficha(nome, caminho):
    return (f'<div class="faixa">{folha(20)}<strong style="font-size:13px;margin-left:2px">{nome}</strong>'
            f'<span class="mono" style="color:var(--t3);min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{caminho}</span></div>')


ACOES = '<div class="acoes"><button class="btn pri">Abrir no programa padrão</button><button class="btn">Mostrar na pasta</button></div>'


def cartao(titulo, nome, caminho, texto, perigo=False):
    return (f'<div class="cartao"><span class="tit">{titulo}</span><div class="doc" style="flex:1">{faixa_ficha(nome, caminho)}'
            f'<div class="ficha-a">{aviso(texto, perigo)}{ACOES}</div></div></div>')


mini_prop = (
    '<aside class="vidro mini-prop" aria-label="Propriedades">'
    '<div class="cab"><span class="tit">Propriedades</span></div>'
    + sec("Pasta de trabalho")
    + dl(("Planilhas", "3 · 3 ocultas"), ("Sistema de datas", "1900"))
    + caixa("Mostrar planilhas ocultas", False) + "</aside>")
vazia_faixa = (f'<div class="folhas" role="tablist" aria-label="Planilhas"><button class="ib pq" disabled="" aria-label="Rolar as planilhas para a esquerda">{pag_ant}</button>'
               f'<button class="ib pq" disabled="" aria-label="Rolar as planilhas para a direita">{pag_prox}</button><div class="fl-rolo"></div></div>')
cartao_vazio = (
    '<div class="cartao"><span class="tit">4 · Sem planilha à vista</span><div style="display:flex;gap:8px;flex:1;min-height:0">'
    f'<div class="doc" style="flex:1;min-width:0">{faixa_ctx("cotacoes.xlsx", [])}'
    f'<div class="ficha-a" style="padding:20px 20px">{aviso("Este arquivo não tem planilhas à vista. Ligue \"Mostrar planilhas ocultas\" nas Propriedades.")}</div>'
    f'{vazia_faixa}{rodape("<span>3 planilhas · todas ocultas</span>")}</div>'
    + mini_prop + "</div></div>")
cartoes = ('<div class="cartoes">'
           + cartao("1 · Arquivo com senha", "folha_pagamento.xlsx", "C:\\rh\\folha_pagamento.xlsx",
                    "Este arquivo está protegido por senha. Tire a proteção no Excel e abra de novo.")
           + cartao("2 · Formato antigo (um .xls com a extensão trocada)", "orcamento_2009.xlsx", "C:\\financeiro\\orcamento_2009.xlsx",
                    "Este é o formato antigo do Excel (.xls, de 1997 a 2003), que o DataBox não lê. Abra o arquivo no Excel e use "
                    "Salvar Como › Pasta de Trabalho do Excel (.xlsx).")
           + cartao("3 · Arquivo estragado", "importacao_out.xlsx", "C:\\temp\\importacao_out.xlsx",
                    "O arquivo não é uma planilha válida: falta a parte «xl/workbook.xml» do arquivo.", perigo=True)
           + cartao_vazio + "</div>")
cena_d = estado(
    "Erros e planilha fora de vista",
    "Erro ao abrir vira a ficha do arquivo com um aviso no lugar do texto \"ainda não está pronto\", com a frase do núcleo (em "
    "português) e os botões que resolvem: Abrir no programa padrão (primário) e Mostrar na pasta. A faixa de contexto e as "
    "Propriedades do arquivo continuam. O arquivo que sumiu usa o mesmo cartão, com \"O arquivo não existe (ou foi movido)\" "
    "e os botões desligados. Erro de uma planilha só (a leitura da aba falhar) mostra o aviso dentro da grade daquela aba, e "
    "as outras continuam. Quando todas as planilhas estão ocultas, a tela abre sem grade, com a faixa das planilhas vazia e as "
    "Propriedades com a caixa Mostrar planilhas ocultas à mão.",
    cartoes, largo=True)

# --- (e) o Trazer colunas com a origem numa planilha -------------------------------------------------------------------
REGIOES_X = [("Sul", "Helena Prado", "410000", "1.280"), ("Sudeste", "Rodrigo Maia", "980000", "4.912"),
             ("Nordeste", "Patrícia Lins", "560000", "2.044"), ("Norte", "Caio Barreto", "220000", "611"),
             ("Centro-Oeste", "Marcos Tavares", "340000", "1.173")]
VENDEDORES = ["Ana Coutinho", "Bruno Brandão", "Carla Ibiapina", "João Esteves", "Marina Lacerda", "Pedro Guimarães"]


def th_p(nome, p, largura=None, classe=""):
    st = f' style="width:{largura}px"' if largura else ""
    return f'<th{f" class=\"{classe}\"" if classe else ""}{st}>{p}{nome}</th>'


def td(valor, classe=""):
    return f'<td{f" class=\"{classe}\"" if classe else ""}>{valor}</td>'


def linha_origem(nome, p, marcada, destino):
    ck = ' checked=""' if marcada else ""
    rotulo = f'<label><input type="checkbox"{ck} aria-label="Trazer {nome}">{p}{nome}</label>'
    if not destino:
        return f'<div class="lt">{rotulo}</div>'
    d = (campo("nova coluna", f"Destino de {nome}", "campo pq")
         + f'<input class="campo pq" value="{destino}" aria-label="Nome da coluna nova para {nome}">')
    return f'<div class="lt on">{rotulo}<div class="dest">{d}</div></div>'


lista_e = ('<div class="lista-tr" role="group" aria-label="Colunas de Regiões">'
           + linha_origem("Gerente", P_TXT, True, "gerente") + linha_origem("Meta Q3 (R$)", P_DEC, True, "meta_regiao")
           + linha_origem("Clientes", P_INT, False, None) + "</div>")
cab_prev = ('<thead><tr><th class="num" style="color:var(--t3);width:40px;padding:0 8px">#</th>'
            + th_p("data", P_DATA, 96) + th_p("regiao", P_TXT, 106) + th_p("vendedor", P_TXT) + th_p("gerente", P_TXT, 130, "tr")
            + th_p("meta_regiao", P_DEC, 112, "num tr") + "</tr></thead>")
vendas_csv = [(2 + i, f"2025-01-{2 + i:02d}", REG[i % 5], VENDEDORES[i % 6]) for i in range(5)]
ger = {r[0]: r for r in REGIOES_X}
corpo_prev = "".join(
    f'<tr><td class="lin">{n}</td>{td(d)}{td(r)}{td(v)}{td(ger[r][1], "tr")}{td(ger[r][2], "num tr")}</tr>' for n, d, r, v in vendas_csv)
prev = '<div class="prev-g"><table class="grade">' + cab_prev + "<tbody>" + corpo_prev + "</tbody></table></div>"
opcao = lambda txt, on: f'<button class="tg" aria-pressed="{"true" if on else "false"}">{CHECK_OP if on else ""}{txt}</button>'
icone20_csv = (f'<span class="sk s20 arq" aria-hidden="true">{lado(doc_csv, 20)}'
               '<span class="rot" style="font-size:6.8px;letter-spacing:0">CSV</span></span>')
dialogo = (
    '<section class="dlg flut tc" role="dialog" aria-label="Trazer colunas para vendas_2025.csv">'
    f'<header>{icone20_csv}<h2 style="margin-left:10px">Trazer colunas</h2><span style="color:var(--t3);margin-left:8px">para vendas_2025.csv</span>'
    f'<span style="flex:1"></span><button class="ib" aria-label="Fechar" title="Fechar (Esc)">{fechar_dlg}</button></header>'
    '<div class="corpo-tc">'
    '<div class="bloco"><span class="tit">De onde</span>'
    '<div class="casar">' + campo("relatorio_q3.xlsx › Regiões", "De onde", icone=folha(16))
    + '<span></span><div class="lado"><span class="meta">6 linhas · 4 colunas</span>'
    '<label class="op" style="color:var(--t2)"><input type="checkbox" checked="" aria-label="Primeira linha é o cabeçalho">Primeira linha é o cabeçalho</label></div></div></div>'
    '<div class="bloco"><span class="tit">Casar as linhas onde</span>'
    '<div class="casar">' + campo("regiao", "Coluna de vendas_2025.csv") + '<span class="igual" aria-hidden="true">=</span>'
    + campo("Região", "Coluna de Regiões")
    + '<span class="meta">deste arquivo</span><span></span><span class="meta">de relatorio_q3.xlsx › Regiões</span></div>'
    '<div class="medida"><div class="txt" aria-live="polite"><strong>1.204 de 1.204 linhas têm par</strong><span>(100%)</span></div>'
    '<div class="medidor" role="meter" aria-label="Linhas com par" aria-valuemin="0" aria-valuemax="1204" aria-valuenow="1204">'
    '<div style="width:100%"></div></div></div>'
    '<div class="opcoes" role="group" aria-label="Como comparar as chaves">'
    + opcao("Ignorar maiúsculas", True) + opcao("Ignorar espaços nas pontas", True) + opcao("Números pelo valor", False) + "</div></div>"
    '<div class="bloco"><div class="cab-bloco"><span class="tit">Trazer</span><span class="meta">2 de 3 colunas</span>'
    '<span style="flex:1"></span><button class="btn sutil pq">Marcar todas</button></div>' + lista_e + "</div>"
    '<div class="bloco"><div class="cab-bloco"><span class="tit">Prévia</span><span class="meta">as 5 primeiras linhas de vendas_2025.csv</span></div>'
    + prev + "</div></div>"
    '<footer><span style="flex:1"></span><button class="btn sutil" title="Esc">Cancelar</button>'
    '<button class="btn pri">Trazer 2 colunas</button></footer></section>')
cena_e = estado(
    "Trazer colunas com origem numa planilha",
    "A planilha entra entre as origens do Trazer colunas do CSV, uma planilha por entrada (\"relatorio_q3.xlsx › Regiões\", "
    "com a folha XLSX). À direita do seletor vão a contagem e a caixa Primeira linha é o cabeçalho, ligada quando as 50 "
    "primeiras linhas parecem ter cabeçalho; desligada, as colunas se chamam A, B, C. O resto é igual ao do CSV: casar, "
    "medidor, o que trazer e a prévia. Os números vão crus, sem milhar nem formato (410000 e não R$ 410.000,00), com a "
    "vírgula do destino; datas e horas vão como a planilha as mostra. Nada é gravado na planilha: ela é só origem.",
    '<div class="fundo-dlg"><div class="esq" aria-hidden="true"></div><div class="veu" aria-hidden="true"></div>' + dialogo + "</div>",
    estilo_palco="padding:0")

# --- (f) o menu do cabeçalho -------------------------------------------------------------------------------------------
LARG_F = [("A", 96), ("B", 170), ("C", 98), ("D", 118), ("E", 84), ("F", 100)]
grade_f = ('<div class="doc"><div class="xlw" style="height:254px;flex:none">'
           + tabela(LARG_F, linhas_vendas(1, 8), selcol="D", ativa=("D", 3)) + "</div>"
           + rodape("<span>1.204 linhas · 6 colunas · A1:F1204</span>", '<span style="color:var(--t3)">· coluna D selecionada</span>') + "</div>")
menu_f1 = ('<div role="menu" aria-label="Coluna D" class="flut menu-ctx largo" style="left:396px;top:60px;width:244px">'
           + mi("Copiar coluna", "Ctrl+C") + mi("Trazer colunas desta planilha", icone=TRAZER) + "</div>")
toast_f = (f'<div role="status" class="flut toast" style="align-self:center;margin-top:8px"><span class="sk s16" aria-hidden="true">{ok_svg}</span>'
           'Coluna D copiada · 1.204 linhas</div>')
menu_f2 = ('<div class="so-menu"><div role="menu" aria-label="Coluna D" class="flut menu-ctx largo" style="left:0;top:4px;width:272px">'
           + mi("Copiar coluna", "Ctrl+C") + mi("Trazer colunas desta planilha", icone=TRAZER, classe="mi off", desligado_=True) + "</div>"
           '<div role="tooltip" class="flut dica-col" style="left:284px;top:36px;width:200px">Abra um CSV para trazer colunas.</div></div>')
menu_f3 = ('<div class="so-menu" style="height:68px"><div role="menu" aria-label="Colunas B, C e D" class="flut menu-ctx largo" style="left:0;top:4px;width:244px">'
           + mi("Copiar 3 colunas", "Ctrl+C") + mi("Trazer colunas desta planilha", icone=TRAZER) + "</div></div>")
cena_f = estado(
    "Menu do cabeçalho",
    "O clique no cabeçalho seleciona a coluna (Ctrl e Shift somam; o clique no número da linha seleciona a linha); o clique "
    "direito ou Shift+F10 abre o menu. Copiar coluna leva o texto da coluna com tabulações, sem cabeçalho (a primeira linha da "
    "planilha é uma linha como as outras) e confirma com o aviso \"Coluna D copiada · 1.204 linhas\"; com várias colunas o item "
    "vira \"Copiar 3 colunas\". Trazer colunas desta planilha abre a janela com a origem já escolhida e só liga quando há um CSV "
    "aberto como destino; sem ele o item fica desligado, com a dica. Não há Colar nem Renomear: a planilha é só leitura.",
    '<div class="etapa" style="position:relative"><span class="tit">1 · Com um CSV aberto</span>' + grade_f + menu_f1 + toast_f + "</div>"
    '<div class="etapa" style="margin-top:6px"><span class="tit">2 · Sem CSV aberto</span>' + menu_f2 + "</div>"
    '<div class="etapa"><span class="tit">3 · Com três colunas selecionadas</span>' + menu_f3 + "</div>")

quadro = '<div class="quadro">' + cena_a + cena_b + cena_c + cena_d + cena_e + cena_f + "</div>"
corpo = (f'<div class="est {{{{classes}}}}" style="width:100%;min-height:{ALTURA}px;position:relative;background:var(--ambiente)">\n'
         + defs + "\n" + quadro + "\n</div>")
script = colunas_[colunas_.index('<script type="text/x-dc"'):]
if not re.search(r'"\$preview":\{"width":1440,"height":\d+\}', script):
    raise SystemExit("planilha: o script do Colunas.dc.html não tem o $preview esperado")
script = re.sub(r'"\$preview":\{"width":1440,"height":\d+\}', f'"$preview":{{"width":1440,"height":{ALTURA}}}', script)
pagina = cabeca + "</helmet>\n" + corpo + "\n</x-dc>\n" + script

destino = PASTA / ARQUIVO
if destino.is_file() and MARCA not in destino.read_text(encoding="utf-8"):
    raise SystemExit(f"planilha: {ARQUIVO} existe sem a marca deste gerador; não gravei nada")

# o lugar no canvas: x 1560, y 8300, sem cobrir nenhuma outra prancheta
lay = PASTA / "canvas.json"
j = json.loads(lay.read_text(encoding="utf-8"))
entrada = entrada_canvas()
for nome, b in j["boards"].items():
    if nome != ARQUIVO and cobre(b, ALTURA):
        raise SystemExit(f"planilha: o lugar da prancheta 17 cobre {nome}; não gravei nada")

if not destino.is_file() or destino.read_text(encoding="utf-8") != pagina:
    destino.write_text(pagina, encoding="utf-8", newline="\n")
    print("gravado:", destino.name, len(pagina), "bytes")
else:
    print("igual:", destino.name)
mudou = j["boards"].get(ARQUIVO) != entrada or ARQUIVO not in j["order"]
j["boards"][ARQUIVO] = entrada
if ARQUIVO not in j["order"]:
    j["order"].append(ARQUIVO)
if mudou:
    lay.write_text(json.dumps(j, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(f"canvas.json: prancheta 17 em x {LUGAR['x']}, y {LUGAR['y']}, {LUGAR['w']} × {ALTURA}")
else:
    print("igual: canvas.json")
