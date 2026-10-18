"""Os ícones do app em Rust (16/10/2026): os SVG que o
rasterizador transforma em PNG e a lista dele.

O que este gerador faz:
- (a) acha cada silhueta nas pranchetas, pelo aria-label, pelo title, pela classe ou pelo texto do botão, e na
  biblioteca (prancheta 9, pelo nome do ladrilho), e grava design/icones/sil/<nome>.svg em branco
  (fill="#ffffff"; o app tinge). Recusa quando não acha, ou quando acha mais de um desenho diferente. As silhuetas
  da fase 8 que já estão em design/icones/ (copiar_caminho, otimizar...) precisam conferir com a prancheta;
- (b) para cada sólido que não está em design/icones/, tira o molde da biblioteca (o <sc-if> da forma no modelo dos
  ladrilhos), liga as camadas opcionais da variante (pena, cadeado, olho, chave, lupa, engrenagem, raio), passa o
  contorno .cont/.cont-p para o desenho pequeno (2,4 e 1,6 até 24 px) ou para o de 48, e chama o exportar.py da
  skill (.claude/skills/icones-databox/scripts/exportar.py) com --cor "{{z.cor}}=#…". Saem <nome>.svg (até 24 pt)
  e <nome>-grande.svg (32 pt para cima; a etiqueta menor nos arquivos);
- (c) escreve design/ferramentas/rasterizar/icones.json no formato do logo.json: o desenho pequeno de cada ícone nos
  tamanhos em pixels (1×, 1,5× e 2× de cada tamanho de uso, mais 1,25× nas silhuetas). Os poucos lugares que mostram
  o desenho -grande (GRANDES: a dock dos contadores da Visão geral, 48 pt, icones_resumo.py) ganham PNG à parte,
  <pasta>/<nome>-g-<px>.png (o "-g" não colide com o <nome>-<px>.png do desenho pequeno); na tabela do Rust a chave
  é "<pasta>/<nome>-g";
- (d) com --tabela, gera crates/interface/src/icones/png.rs com os PNG que existem em crates/interface/assets/icones
  (rode depois do git mv do passo 8, que cria a pasta icones/);
- com --conferir, não grava nada: diz o que falta ou está diferente (SVG, lista, PNG no tamanho do nome e com o canto
  transparente, PNG sobrando na pasta) e sai com erro se houver alguma coisa.

Pode rodar de novo: o que já está igual fica; o que ele mesmo gravou antes (a marca data-gerador="icones_app.py" no
<svg>, ou a primeira linha do png.rs) é refeito; um arquivo sem a marca e diferente faz o gerador parar sem gravar
nada, para não apagar um ajuste feito à mão.

Os PNG saem do rasterizador do repositório, da raiz:
  python -I design/ferramentas/rasterizar/servidor.py crates/interface/assets 8790   (em segundo plano)
  chrome.exe --headless=new --disable-gpu --hide-scrollbars --user-data-dir=<pasta temporária nova>
      --virtual-time-budget=1000000 --dump-dom "http://127.0.0.1:8790/ferramentas/rasterizar/rasterizar.html?lista=icones.json"
O DOM impresso traz o título "pronto: N arquivos", com N igual ao número de PNG da lista (com 20000 de orçamento a
página para no meio: são mais de 500 PNG). Depois pare o servidor e rode o --conferir.

Uso: python -I -X utf8 design/ferramentas/icones_app.py [--canvas PASTA] [--conferir | --tabela [SAIDA]]
Só biblioteca padrão."""
import argparse
import json
import re
import struct
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
import zlib
from collections import defaultdict
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8")
RAIZ = Path(__file__).resolve().parents[2]
ICONES = RAIZ / "design" / "icones"
SIL = ICONES / "sil"
LISTA = RAIZ / "design" / "ferramentas" / "rasterizar" / "icones.json"
ASSETS = RAIZ / "crates" / "interface" / "assets"
TABELA = RAIZ / "crates" / "interface" / "src" / "icones" / "png.rs"
EXPORTAR = RAIZ / ".claude" / "skills" / "icones-databox" / "scripts" / "exportar.py"
MARCA = 'data-gerador="icones_app.py"'
MARCA_RS = "// Gerado por design/ferramentas/icones_app.py --tabela. Não edite à mão."

# Silhuetas: 16 pt na barra, 14 pt nos botões pequenos e 12 pt no cabeçalho dos painéis, em 1×, 1,25×, 1,5× e 2×.
SIL_PX = [12, 14, 15, 16, 18, 20, 21, 24, 28, 32]

# (arquivo, nome do ladrilho na biblioteca, como achar nas pranchetas: aria-label, title, classe ou texto do botão)
SILHUETAS = [
    ("voltar", "Voltar", ("Voltar", "Página anterior")),
    ("avancar", "Avançar", ("Avançar", "Próxima página")),
    ("pasta", "Abrir pasta", ("Abrir pasta",)),
    ("arquivo", "Abrir arquivo", ("Abrir arquivo",)),
    ("salvar", "Salvar", ("Salvar",)),
    ("novo", "Novo", ("Novo", "Nova linha", "Nova tabela")),
    ("nova_coluna", "Nova coluna", ("Nova coluna", "Nova coluna depois desta")),
    ("duplicar", "Duplicar", ("Duplicar",)),
    ("excluir", "Excluir", ("Excluir", "Excluir a coluna")),
    ("desfazer", "Desfazer", ("Desfazer",)),
    ("refazer", "Refazer", ("Refazer",)),
    ("buscar", "Buscar", ("campo busca",)),
    ("filtrar", "Filtrar", ("Filtrar",)),
    ("atualizar", "Atualizar", ("Atualizar", "Recarregar")),
    ("executar", "Executar", ("Executar",)),
    ("interromper", "Interromper consulta", ("Interromper",)),
    ("nova_consulta", "Nova consulta", ("Nova consulta",)),
    ("exportar", "Exportar", ("Exportar",)),
    ("painel", "Painel de propriedades", ("Painel de propriedades",)),
    ("recolher", "Recolher tudo", ("Recolher tudo",)),
    ("mostrar_na_pasta", "Mostrar na pasta", ("Mostrar na pasta",)),
    ("copiar_caminho", "Copiar caminho", ("Copiar caminho",)),
    ("editar", "Renomear", ("Renomear a tabela", "Renomear a coluna", "Alterar a coluna")),
    ("subir", "Subir", ("Subir",)),
    ("descer", "Descer", ("Descer",)),
    ("inserir", "Inserir no editor", ("q",)),
    ("verificar_integridade", "Verificar integridade", ("Verificar integridade",)),
    ("verificar_chaves", "Verificar chaves estrangeiras", ("Verificar chaves estrangeiras",)),
    ("otimizar", "Otimizar", ("Otimizar",)),
    ("compactar", "Compactar (VACUUM)", ("Compactar (VACUUM)",)),
    ("copia_seguranca", "Fazer cópia de segurança", ("Fazer cópia de segurança",)),
    ("inicio", "Início", ("Início",)),
    ("novo_banco", "Novo banco", ("Novo banco",)),
    ("trazer_colunas", "Trazer colunas", ("Trazer colunas", "Trazer colunas de outro arquivo")),
]

# Sólidos: (pasta dos PNG, nome do PNG, arquivo em design/icones, origem, tamanhos em pixels). A origem é None para
# os da fase 8, que já estão em design/icones, ou (forma da biblioteca, cor, camadas ligadas).
TRILHO, MENU, TABELAS, PARTES, SECOES = [22, 33, 44], [24, 36, 48], [16, 20, 24, 30, 32, 40], [16, 24, 32], [20, 30, 40]
SOLIDOS = [
    ("solidos", "inicio", "inicio", None, TRILHO),
    ("solidos", "explorador", "explorador", None, TRILHO),
    ("solidos", "historico", "historico", None, TRILHO),
    ("solidos", "preferencias", "preferencias", None, [20, 22, 24, 30, 33, 36, 40, 44, 48]),
    ("solidos", "documento", "documento", None, MENU),
    ("solidos", "novo_banco", "novo_banco", None, MENU),
    ("solidos", "salvar", "salvar", None, MENU),
    ("solidos", "pasta", "pasta", ("ePasta", None, ()), [16, 24, 32, 36, 48]),
    ("solidos", "recentes", "recentes", ("a_recentes", None, ()), [16, 24, 32, 36, 48]),
    ("solidos", "banco", "banco", ("eCil", "#2c6fdc", ("Pena",)), [16, 20, 24, 30, 32, 36, 40, 48]),
    ("solidos", "banco_so_leitura", "banco_so_leitura", ("eCil", "#8a919c", ("Pena", "Cadeado")),
     [16, 20, 24, 30, 32, 36, 40, 48]),
    ("solidos", "tabela", "tabela", ("eTab", "#2c6fdc", ()), TABELAS),
    ("solidos", "tabela_sem_rowid", "tabela_sem_rowid", None, TABELAS),
    ("solidos", "tabela_virtual", "tabela_virtual", ("eTab", "#1f8a8a", ("Lupa",)), TABELAS),
    ("solidos", "visao", "visao", ("eTab", "#7a52c7", ("Olho",)), TABELAS),
    ("solidos", "sistema", "sistema", ("eTab", "#6b7280", ("Engr",)), TABELAS),
    ("solidos", "dados", "dados", ("eGrade", None, ()), PARTES),
    ("solidos", "estrutura", "estrutura", ("eEstr", None, ()), PARTES),
    ("solidos", "indices", "indices", ("eInd", "#2c6fdc", ("RaioInd",)), PARTES),
    ("solidos", "indice", "indice", ("eInd", "#2c6fdc", ()), PARTES),
    ("solidos", "gatilho", "gatilho", ("eRaio", None, ()), PARTES),
    # disco e linhas, dos contadores Tamanho e Linhas da Visão geral (icones_resumo.py); a dock usa o -grande (GRANDES)
    ("solidos", "disco", "disco", ("a_disco", None, ()), PARTES),
    ("solidos", "linhas", "linhas", ("a_linhas", None, ()), PARTES),
    ("solidos", "geral", "geral", None, SECOES),
    ("solidos", "aparencia", "aparencia", None, SECOES),
    ("solidos", "exportacao", "exportacao", None, SECOES),
    ("folhas", "sql", "folha_sql", ("eDoc", "#2c6fdc", ()), TABELAS),
    ("folhas", "xlsx", "folha_xlsx", ("eDoc", "#1d7443", ()), PARTES),
    ("folhas", "csv", "folha_csv", ("eDoc", "#27854c", ()), PARTES),
    ("folhas", "json", "folha_json", ("eDoc", "#9c6c0c", ()), PARTES),
    ("folhas", "txt", "folha_txt", ("eDoc", "#6f7783", ()), PARTES),
]
TIPOS = [("inteiro", "#2c6fdc"), ("real", "#1d8181"), ("texto", "#5f6b7a"), ("blob", "#7a52c7"),
         ("data", "#c8443b"), ("booleano", "#27854c"), ("json", "#9c6c0c"), ("chave_primaria", "#927010"),
         ("chave_estrangeira", "#4a6fd1"), ("nulo", "#6f757f")]
SOLIDOS += [("tipos", n, f"tipo_{n}", ("eTipo", c, ()), [16, 24, 32, 36, 48]) for n, c in TIPOS]
ESTADOS = [("atencao", "st_atencao"), ("erro", "st_erro"), ("concluido", "st_ok"), ("informacao", "st_info"),
           ("ajuda", "st_ajuda"), ("bloqueado", "st_bloqueado")]
SOLIDOS += [("estados", n, f"estado_{n}", (f, None, ()), [14, 16, 21, 24, 28, 32, 36, 48]) for n, f in ESTADOS]
# A forma de 32 pt para cima, quando muda (a etiqueta menor nos arquivos).
GRANDE = {"eDoc": "eDocG", "a_disco": "a_discoG", "a_linhas": "a_linhasG"}
# O desenho -grande rasterizado à parte: (pasta, nome do PNG, arquivo em design/icones sem o "-grande", tamanhos).
# A dock dos contadores da Visão geral mostra os seis a 48 pt (icones_resumo.py): 48, 72 e 96 px.
DOCK = [48, 72, 96]
GRANDES = [("solidos", n, n, DOCK) for n in ("tabela", "visao", "indice", "gatilho", "disco", "linhas")]
for _g in GRANDES:  # o -grande.svg sai da biblioteca pelo (b): a origem do sólido não pode ser None
    if not any(s[2] == _g[2] and s[3] is not None for s in SOLIDOS):
        raise SystemExit(f"icones_app: {_g[2]}-grande.svg não sai da biblioteca (veja SOLIDOS)")
CAMADAS = ("Cadeado", "Olho", "Chave", "Lupa", "Engr", "Pena", "RaioInd", "Golfinho", "Elefante", "Pato", "Folha")


def parar(msg):
    raise SystemExit("icones_app: " + msg)


def norm(s):
    return re.sub(r"\s+", " ", s).strip()


# ---------- (a) silhuetas ----------

def biblioteca_silhuetas(icones_html):
    m = re.search(r'<section class="painel" aria-label="Silhuetas da barra de ferramentas">(.*?)</section>', icones_html, re.S)
    if not m:
        parar("a prancheta 9 não tem a seção Silhuetas da barra de ferramentas")
    lib = {}
    for tile in m.group(1).split('<div class="tile">')[1:]:
        nome = re.search(r'<div class="nome">(.*?)</div>', tile).group(1)
        desenhos = {norm(d) for d in re.findall(r'<svg class="sil"[^>]*>(.*?)</svg>', tile, re.S)}
        if len(desenhos) != 1:
            parar(f"o ladrilho {nome!r} da biblioteca tem {len(desenhos)} desenhos diferentes")
        if nome in lib:
            parar(f"o ladrilho {nome!r} aparece duas vezes na biblioteca")
        lib[nome] = desenhos.pop()
    return lib


def silhuetas_nas_pranchetas(canvas):
    """chave (aria-label, title, classe ou texto) -> desenho -> pranchetas"""
    achados = defaultdict(lambda: defaultdict(set))
    for arq in sorted(canvas.glob("*.dc.html")):
        if arq.name in ("Icones.dc.html", "TrilhoOpcoes.dc.html"):  # a biblioteca e as opções do trilho
            continue
        t = arq.read_text(encoding="utf-8")
        for m in re.finditer(r'<svg class="sil"[^>]*>(.*?)</svg>', t, re.S):
            antes = re.findall(r"<(?:button|a|span|div|label)\b[^>]*>", t[max(0, m.start() - 600):m.start()])
            pai = antes[-1] if antes else ""
            chaves = set()
            for atr in ("aria-label", "title", "class"):
                a = re.search(rf'\b{atr}="([^"]*)"', pai)
                if a:
                    chaves |= {a.group(1), re.sub(r"\s*\(.*\)$", "", a.group(1))}
            depois = re.match(r"\s*(?:<[^>]+>\s*)*([^<]{0,60})", t[m.end():])
            if depois and depois.group(1).strip():
                chaves.add(depois.group(1).strip())
            for c in chaves:
                achados[c][norm(m.group(1))].add(arq.name)
    return achados


def svg_silhueta(desenho):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" fill="#ffffff" '
            f'{MARCA}>{desenho}</svg>\n')


def silhuetas(canvas, icones_html):
    lib = biblioteca_silhuetas(icones_html)
    nas = silhuetas_nas_pranchetas(canvas)
    saidas = {}
    for nome, ladrilho, chaves in SILHUETAS:
        if ladrilho not in lib:
            parar(f"a silhueta {nome!r} não está na biblioteca (ladrilho {ladrilho!r})")
        desenhos = defaultdict(set)
        desenhos[lib[ladrilho]].add("Icones.dc.html")
        achou = False
        for c in chaves:
            for d, onde in nas.get(c, {}).items():
                desenhos[d] |= onde
                achou = True
        if not achou:
            parar(f"a silhueta {nome!r} não foi achada nas pranchetas por {', '.join(chaves)}")
        if len(desenhos) > 1:
            lista = "; ".join(f"{d[:60]}… em {', '.join(sorted(o))}" for d, o in desenhos.items())
            parar(f"a silhueta {nome!r} tem {len(desenhos)} desenhos diferentes: {lista}")
        desenho = lib[ladrilho]
        fase8 = ICONES / f"{nome}.svg"
        if fase8.is_file():
            t = fase8.read_text(encoding="utf-8")
            if 'viewBox="0 0 24 24"' in t:
                interno = re.search(r"<svg\b[^>]*>(.*)</svg>", t, re.S).group(1)
                if norm(interno) != desenho:
                    parar(f"design/icones/{nome}.svg (fase 8) não confere com a silhueta das pranchetas")
        saidas[SIL / f"{nome}.svg"] = svg_silhueta(desenho)
    return saidas


# ---------- (b) sólidos ----------

def moldes(icones_html):
    if icones_html.count('<sc-for list="{{secoes}}"') != 1:
        parar("a prancheta 9 não tem um modelo de seções só")
    i = icones_html.index('<sc-for list="{{t.tamanhos}}"', icones_html.index('<sc-for list="{{secoes}}"'))
    f = icones_html.index('<span class="rot">{{z.texto}}</span>', i)
    formas = {}
    for m in re.finditer(r'<sc-if value="\{\{z\.(\w+)\}\}"[^>]*>(<svg\b.*?</svg>)</sc-if>', icones_html[i:f], re.S):
        if m.group(1) in formas:
            parar(f"a forma {m.group(1)} aparece duas vezes no modelo dos ladrilhos")
        formas[m.group(1)] = m.group(2)
    return formas


def tirar_elemento(svg, inicio):
    """Tira o <g> que começa em `inicio`, com tudo o que ele tem dentro."""
    nivel = 0
    for m in re.compile(r"<(/?)g\b[^>]*>").finditer(svg, inicio):
        nivel += -1 if m.group(1) else 1
        if nivel == 0:
            return svg[:inicio] + svg[m.end():]
    parar("um <g> do molde não fecha")


def resolver(molde, ligadas, grande):
    if 'width="{{z.s}}" height="{{z.s}}"' not in molde:
        parar("o molde não tem o tamanho {{z.s}}")
    svg = molde.replace('width="{{z.s}}" height="{{z.s}}"', 'width="48" height="48"', 1).replace(' aria-hidden="true"', "", 1)
    for c in ligadas:
        if f'class="{{{{z.cls{c}}}}}"' not in svg:
            parar(f"o molde não tem a camada {c}")
    while True:
        m = re.search(r'<g class="\{\{z\.cls(\w+)\}\}">', svg)
        if not m:
            break
        if m.group(1) not in CAMADAS:
            parar(f"camada desconhecida no molde: {m.group(1)}")
        if m.group(1) in ligadas:
            svg = svg[:m.start()] + "<g>" + svg[m.end():]
        else:
            svg = tirar_elemento(svg, m.start())

    def contorno(mm):
        tag = mm.group(0)
        cls = re.search(r' class="(cont(?:-p)?)"', tag).group(1)
        tag = tag.replace(f' class="{cls}"', "", 1)
        if grande:
            return tag
        opac, larg = (".35", "2.4") if cls == "cont" else (".25", "1.6")
        st = re.search(r' style="([^"]*)"', tag)
        resto = [p for p in (st.group(1).split(";") if st else []) if p and not p.startswith(("stroke-opacity", "stroke-width"))]
        estilo = f' style="stroke-opacity:{opac};stroke-width:{larg}' + "".join(";" + p for p in resto) + '"'
        if st:
            return tag.replace(st.group(0), estilo, 1)
        return re.sub(r"\s*/?>$", lambda f: estilo + f.group(0).strip(), tag)

    svg = re.sub(r'<[a-zA-Z]+\b[^>]* class="cont(?:-p)?"[^>]*>', contorno, svg)
    sobra = re.findall(r"\{\{(?!z\.cor\}\})[^}]*\}\}|\bclass=\"[^\"]*\"", svg)
    if sobra:
        parar(f"o molde ficou com buracos desconhecidos: {sorted(set(sobra))}")
    return svg


def exportar(svg, cor, tmp, n):
    if ("{{z.cor}}" in svg) != (cor is not None):
        parar("a cor do molde e a da variante não combinam")
    entrada, saida = tmp / f"m{n}.svg", tmp / f"s{n}.svg"
    entrada.write_text(svg, encoding="utf-8", newline="\n")
    cmd = [sys.executable, "-I", "-X", "utf8", str(EXPORTAR), str(entrada), str(saida)]
    if cor:
        cmd += ["--cor", "{{z.cor}}=" + cor]
    r = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        parar(f"o exportar.py falhou: {r.stderr.strip() or r.stdout.strip()}")
    t = saida.read_text(encoding="utf-8")
    return t.replace("<svg ", f"<svg {MARCA} ", 1) if MARCA not in t else t


def solidos(icones_html, tmp):
    formas = moldes(icones_html)
    saidas, n = {}, 0
    for _pasta, _png, arquivo, origem, _px in SOLIDOS:
        if origem is None:
            if not (ICONES / f"{arquivo}.svg").is_file():
                parar(f"falta design/icones/{arquivo}.svg (da fase 8)")
            continue
        forma, cor, ligadas = origem
        for grande, sufixo in ((False, ""), (True, "-grande")):
            f = GRANDE.get(forma, forma) if grande else forma
            if f not in formas:
                parar(f"a biblioteca não tem a forma {f}")
            n += 1
            saidas[ICONES / f"{arquivo}{sufixo}.svg"] = exportar(resolver(formas[f], ligadas, grande), cor, tmp, n)
    return saidas


# ---------- (c) a lista do rasterizador ----------

def itens_da_lista():
    itens = [{"svg": f"../../icones/sil/{n}.svg", "tamanhos": SIL_PX, "saida": f"icones/silhuetas/{n}-{{t}}.png"}
             for n, _l, _c in SILHUETAS]
    itens += [{"svg": f"../../icones/{arq}.svg", "tamanhos": px, "saida": f"icones/{pasta}/{png}-{{t}}.png"}
              for pasta, png, arq, _o, px in SOLIDOS]
    itens += [{"svg": f"../../icones/{arq}-grande.svg", "tamanhos": px, "saida": f"icones/{pasta}/{png}-g-{{t}}.png"}
              for pasta, png, arq, px in GRANDES]
    return itens


def texto_da_lista():
    return "[\n" + ",\n".join("  " + json.dumps(i, ensure_ascii=False) for i in itens_da_lista()) + "\n]\n"


def pngs_esperados():
    return {i["saida"].replace("{t}", str(t)): t for i in itens_da_lista() for t in i["tamanhos"]}


# ---------- gravar ----------

def conferir_svg(caminho, texto):
    try:
        ET.fromstring(texto)
    except ET.ParseError as e:
        parar(f"{caminho.name} não é um SVG válido: {e}")


def gravar(saidas):
    """Grava tudo ou nada: para antes se algum arquivo existe sem a marca e com outro conteúdo."""
    for caminho, texto in saidas.items():
        if caminho.suffix == ".svg":
            conferir_svg(caminho, texto)
        if caminho.is_file():
            atual = caminho.read_text(encoding="utf-8")
            dono = MARCA in atual or caminho == LISTA or atual.startswith(MARCA_RS)
            if atual != texto and not dono:
                parar(f"{caminho.relative_to(RAIZ)} existe, é diferente e não foi gravado por este gerador")
    mudou = 0
    for caminho, texto in saidas.items():
        if caminho.is_file() and caminho.read_text(encoding="utf-8") == texto:
            continue
        caminho.parent.mkdir(parents=True, exist_ok=True)
        caminho.write_text(texto, encoding="utf-8", newline="\n")
        mudou += 1
        print("gravado:", caminho.relative_to(RAIZ))
    return mudou


# ---------- PNG ----------

def ler_png(caminho):
    """(largura, altura, alfa do canto de cima à esquerda ou None)"""
    b = caminho.read_bytes()
    if not b.startswith(b"\x89PNG\r\n\x1a\n") or b[12:16] != b"IHDR":
        return None
    larg, alt, prof, tipo = struct.unpack(">IIBB", b[16:26])
    alfa = None
    if prof == 8 and tipo == 6:
        dados, pos = b"", 8
        while pos < len(b):
            n, nome = struct.unpack(">I4s", b[pos:pos + 8])
            if nome == b"IDAT":
                dados += b[pos + 8:pos + 8 + n]
            pos += 12 + n
        linha = zlib.decompressobj().decompress(dados, 1 + 4 * larg)
        alfa = linha[4]  # o primeiro pixel não tem vizinho: qualquer filtro o deixa como está
    return larg, alt, alfa


def conferir(saidas):
    problemas = []
    for caminho, texto in saidas.items():
        if not caminho.is_file():
            problemas.append(f"falta {caminho.relative_to(RAIZ)}")
        elif caminho.read_text(encoding="utf-8") != texto:
            problemas.append(f"diferente: {caminho.relative_to(RAIZ)} (rode o gerador)")
    esperados = pngs_esperados()
    for rel, t in sorted(esperados.items()):
        p = ASSETS / rel
        if not p.is_file():
            problemas.append(f"falta o PNG {rel}")
            continue
        info = ler_png(p)
        if info is None:
            problemas.append(f"não é PNG: {rel}")
        elif info[:2] != (t, t):
            problemas.append(f"{rel} tem {info[0]}×{info[1]}, e não {t}×{t}")
        elif info[2] not in (0, None):
            problemas.append(f"{rel} não tem o canto transparente")
    pasta = ASSETS / "icones"
    if pasta.is_dir():
        for p in sorted(pasta.rglob("*")):
            if p.is_file() and p.relative_to(ASSETS).as_posix() not in esperados:
                problemas.append(f"sobrando: {p.relative_to(ASSETS).as_posix()} (não está na lista)")
    for p in problemas:
        print(p)
    n_svg = sum(1 for c in saidas if c.suffix == ".svg") + sum(1 for s in SOLIDOS if s[3] is None)
    if problemas:
        raise SystemExit(f"icones_app --conferir: {len(problemas)} problema(s)")
    print(f"tudo certo: {n_svg} SVG, {len(itens_da_lista())} linhas no icones.json e {len(esperados)} PNG")


# ---------- (d) a tabela do Rust ----------

def tabela(saida):
    if not saida.parent.is_dir():
        parar(f"a pasta {saida.parent} não existe (o passo 8 cria com o git mv do icones.rs)")
    linhas = [MARCA_RS, "//! Os PNG de `crates/interface/assets/icones`, por ícone (`pasta/nome`) e do menor para o maior.", "",
              "/// (`pasta/nome`, [(lado em pixels, bytes do PNG)])",
              "pub(crate) static PNG: &[(&str, &[(u32, &[u8])])] = &["]
    base = Path("../" * (len(saida.relative_to(RAIZ / "crates" / "interface").parts) - 1))
    for item in itens_da_lista():
        chave = item["saida"][len("icones/"):-len("-{t}.png")]
        existem = [t for t in item["tamanhos"] if (ASSETS / item["saida"].replace("{t}", str(t))).is_file()]
        if not existem:
            continue
        linhas.append(f'    ("{chave}", &[')
        for t in existem:
            caminho = (base / "assets" / item["saida"].replace("{t}", str(t))).as_posix()
            linhas.append(f'        ({t}, include_bytes!("{caminho}")),')
        linhas.append("    ]),")
    linhas.append("];")
    return {saida: "\n".join(linhas) + "\n"}


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--canvas", type=Path, default=RAIZ / "design" / "canvas")
    g = p.add_mutually_exclusive_group()
    g.add_argument("--conferir", action="store_true")
    g.add_argument("--tabela", nargs="?", type=Path, const=TABELA)
    a = p.parse_args()
    if a.tabela:
        saida = a.tabela.resolve()
        if not saida.is_relative_to(RAIZ / "crates" / "interface"):
            parar("a tabela fica dentro de crates/interface")
        mudou = gravar(tabela(saida))
        print(f"tabela: {mudou} arquivo(s) gravado(s)")
        return
    icones_html = (a.canvas / "Icones.dc.html").read_text(encoding="utf-8")
    with tempfile.TemporaryDirectory(prefix="icones_app_") as tmp:
        saidas = silhuetas(a.canvas, icones_html)
        saidas |= solidos(icones_html, Path(tmp))
    saidas[LISTA] = texto_da_lista()
    if a.conferir:
        conferir(saidas)
        return
    mudou = gravar(saidas)
    print(f"{mudou} arquivo(s) gravado(s), {len(saidas) - mudou} já estavam iguais; "
          f"{len(pngs_esperados())} PNG na lista {LISTA.relative_to(RAIZ)}")


if __name__ == "__main__":
    main()
