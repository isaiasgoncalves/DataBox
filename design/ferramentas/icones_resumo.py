"""Ícones nos contadores da Visão geral (prancheta 1, Main.dc.html), pedido do Isaías em 17/10/2026: "para o design,
coloque ícones em cada um dos boxes dessa tela". Os seis contadores (Tabelas, Visão, Índices, Gatilho, Tamanho,
Linhas) ganham um sólido de 16 px à esquerda do rótulo, embaixo do número: o número continua com a largura toda da
célula (os valores do app, como "496.482" e "119 MB", não cabem ao lado de um ícone em células de ~130 px) e a faixa
cresce só 3 px. Os quatro primeiros são os da biblioteca (tabela, visão, índice e gatilho); os dois últimos são novos:
- disco: o disco rígido aberto, de frente (caixa de metal, o prato com o eixo e o braço da cabeça de leitura), para
  Tamanho. É uma caixa retangular com um círculo, nada do cilindro de lado do banco;
- linhas: a folha com cinco linhas listradas e a do meio em azul, saindo um pouco da folha dos dois lados, para
  Linhas. Sem as divisas verticais da tabela e dos dados e sem as barras de larguras diferentes do índice.
Cada um tem dois desenhos: o de até 24 px (contorno grosso pela classe .cont, só as peças que se leem) e o de 48
(parafusos, brilho no prato, divisas e o "texto" das linhas). Os dois entram na biblioteca (prancheta 9, seção
Aplicativo) como as formas a_disco/a_discoG e a_linhas/a_linhasG, que o icones_app.py exporta para design/icones.


Segunda rodada, no mesmo dia (comentário do Isaías na faixa: "Os ícones aqui podem ficar bem maiores, pode usar a
maior versão, exiba eles elegantemente, se quiser pode até colocar eles em cima de uma 'dock'"): os contadores viram
uma dock. Cada um mostra o desenho -grande do sólido (o de 48, com contorno fino, parafusos, divisas e o "texto" das
linhas) a 48 px, em pé sobre uma prateleira de 82 px de altura (raio 10, degradê --dock1 → --dock2 com um brilho nos
22 px de cima, fio --dockb, luz de 1 px em cima e sombra macia embaixo) que começa 26 px abaixo do topo do ícone; o
número (24/300) e o rótulo (.tit) ficam centrados embaixo, dentro da prateleira. O pé de cada ícone (a borda de baixo
do desenho na grade 48: tabela 40,5, visão/índice/gatilho 42,5, disco 44,5, linhas 43,5) desce até a mesma linha
(44,5): as classes pe1/pe2/pe4 baixam o desenho 1, 2 ou 4 px. Ali ficam a sombra de contato (elipse de 36 × 9) e um
reflexo fraco (o pé espelhado, até 20% e sumindo em 8 px). No hover o ícone sobe 3 px e a sombra estreita. Abaixo de
700 px de largura (container query), a dock quebra em duas prateleiras de três, separadas por 14 px.
Variantes rejeitadas: ladrilho por contador (seis caixas que parecem botões, pesadas ao lado da lista de tabelas) e o
ícone de 40 à esquerda do número (em seis células de ~130 px o "496.482" é cortado).

Pode rodar de novo: a biblioteca que já tem as quatro formas fica como está, e os contadores com a marca
data-icones="resumo" (a faixa de 16 px da primeira rodada ou a dock) são refeitos a partir dos moldes da biblioteca,
guardando o número e o rótulo de cada um; o CSS da dock fica entre os comentários CSS_INI e CSS_FIM e é trocado
inteiro (só grava se mudou). Recusa o que não conhece: biblioteca com só parte das formas, contador fora dos formatos
conhecidos, rótulos fora da ordem esperada, CSS com só um dos comentários, ou url(#…) sem definição na prancheta.
Os PNG da dock (solidos/<nome>-g-48/72/96.png, do -grande.svg) estão na lista do icones_app.py.
Uso: python -I -X utf8 icones_resumo.py [pasta das pranchetas]"""
import re
import sys
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8")
AQUI = Path(__file__).resolve().parent
PASTA = Path(sys.argv[1]) if len(sys.argv) > 1 else AQUI.parent / "canvas"
sys.path.insert(0, str(AQUI))
import icones_app  # noqa: E402  (moldes() e resolver(): o mesmo caminho que leva os ícones ao app)

MARCA = 'data-icones="resumo"'
LADO = 48  # o desenho -grande em 1:1 com a grade


def parar(msg):
    raise SystemExit("icones_resumo: " + msg)


# ---------- os desenhos novos (viewBox 48) ----------

DISCO_DEFS = ('<defs><linearGradient id="disco_corpo" x1="0" y1="0" x2="0" y2="1">'
              '<stop offset="0" style="stop-color:#e4e8ee"></stop><stop offset="1" style="stop-color:#8994a3"></stop></linearGradient>'
              '<radialGradient id="disco_prato" cx=".4" cy=".36" r=".72"><stop offset="0" style="stop-color:#ffffff"></stop>'
              '<stop offset=".55" style="stop-color:#d7dde5"></stop><stop offset="1" style="stop-color:#8f99a8"></stop></radialGradient></defs>')


def disco(grande):
    """Caixa x 7–41, y 4–44; prato r 13,5 em (24, 20); braço do pivô (33, 38) até a cabeça em (26,5, 26)."""
    p = [DISCO_DEFS,
         '<rect class="cont" filter="url(#icSombra)" x="7" y="4" width="34" height="40" rx="4.5" fill="url(#disco_corpo)" stroke="#000000" style="stroke-opacity:.3"></rect>',
         '<rect x="7.5" y="4.5" width="33" height="9" rx="4" fill="url(#icRealce)"></rect>']
    if grande:  # três parafusos (o quarto fica sob o pivô)
        p += [f'<circle cx="{x}" cy="{y}" r="1.1" fill="#6f7886" stroke="#000000" style="stroke-opacity:.25;stroke-width:.6"></circle>'
              for x, y in ((10.5, 7.5), (37.5, 7.5), (10.5, 40.5))]
    p.append('<circle cx="24" cy="20" r="13.5" fill="url(#disco_prato)" stroke="#000000" style="stroke-opacity:.3;stroke-width:'
             + ("1" if grande else "1.6") + '"></circle>')
    p.append('<circle cx="24" cy="20" r="8.5" fill="none" stroke="#000000" style="stroke-opacity:.12"></circle>')
    if grande:
        p.append('<path d="M14.6 14.6a11 11 0 0 1 7.9-5.1" fill="none" stroke="#ffffff" style="stroke-opacity:.85;stroke-width:1.2;stroke-linecap:round"></path>')
    p.append('<circle cx="24" cy="20" r="3.4" fill="url(#icTampa)" stroke="#000000" style="stroke-opacity:.35"></circle>')
    p.append('<path d="M33 38 26.5 26" fill="none" stroke="#353b45" style="stroke-width:3;stroke-linecap:round"></path>')
    if grande:
        p.append('<path d="M32.6 36.6 27.4 27" fill="none" stroke="#ffffff" style="stroke-opacity:.3;stroke-width:.8;stroke-linecap:round"></path>')
    p.append('<circle cx="33" cy="38" r="3" fill="#5d6673" stroke="#000000" style="stroke-opacity:.35"></circle>')
    p.append('<circle cx="32.3" cy="37.2" r="1" fill="#ffffff" style="fill-opacity:.45"></circle>')
    return p


LINHAS_DEFS = ('<defs><linearGradient id="linhas_faixa" x1="0" y1="0" x2="0" y2="1"><stop offset="0" style="stop-color:#5b97f2"></stop>'
               '<stop offset="1" style="stop-color:#1f56b8"></stop></linearGradient></defs>')


def linhas(grande):
    """Folha x 8–40, y 5–43, cinco linhas de 7,6; a terceira (y 20,2–27,8) vira a faixa azul x 4–44, com sombra."""
    p = [LINHAS_DEFS,
         '<rect class="cont" filter="url(#icSombra)" x="8" y="5" width="32" height="38" rx="3.5" fill="url(#icPapel)" stroke="#000000" style="stroke-opacity:.3"></rect>',
         '<path d="M8 12.6h32v7.6H8zM8 27.8h32v7.6H8z" fill="#000000" style="fill-opacity:.1"></path>']
    if grande:
        p.append('<path d="M8 12.6h32M8 20.2h32M8 27.8h32M8 35.4h32" stroke="#000000" style="stroke-opacity:.12;stroke-width:1"></path>')
        p.append('<path d="M12 8.8h5M20.5 8.8h14M12 16.4h5M20.5 16.4h10M12 31.6h5M20.5 31.6h12M12 39.2h5M20.5 39.2h9" stroke="#000000" style="stroke-opacity:.2;stroke-width:1.8;stroke-linecap:round"></path>')
    else:
        p.append('<path d="M8 12.6h32M8 35.4h32" stroke="#000000" style="stroke-opacity:.16;stroke-width:1.4"></path>')
    p.append('<rect filter="url(#icSombra)" x="4" y="19.4" width="40" height="9.2" rx="2.6" fill="url(#linhas_faixa)" stroke="#000000" style="stroke-opacity:.35"></rect>')
    p.append('<rect x="4.6" y="20" width="38.8" height="3.6" rx="1.8" fill="url(#icRealce)"></rect>')
    if grande:
        p.append('<path d="M12 24h5M20.5 24h13" stroke="#ffffff" style="stroke-opacity:.9;stroke-width:1.8;stroke-linecap:round"></path>')
    return p


def molde(partes):
    return '<svg viewBox="0 0 48 48" width="{{z.s}}" height="{{z.s}}" aria-hidden="true">' + "".join(partes) + "</svg>"


NOVAS = {"a_disco": molde(disco(False)), "a_discoG": molde(disco(True)),
         "a_linhas": molde(linhas(False)), "a_linhasG": molde(linhas(True))}


# ---------- prancheta 9: a biblioteca ----------

def trocar(texto, antigo, novo):
    if texto.count(antigo) != 1:
        parar(f"a biblioteca não tem uma vez só: {antigo[:70]!r}")
    return texto.replace(antigo, novo)


def biblioteca(t):
    tem = [f for f in NOVAS if f'<sc-if value="{{{{z.{f}}}}}"' in t]
    if len(tem) == len(NOVAS):
        for f, m in NOVAS.items():
            if f'<sc-if value="{{{{z.{f}}}}}" hint-placeholder-val="{{{{ false }}}}">{m}</sc-if>' not in t:
                parar(f"a forma {f} da biblioteca foi mudada à mão; acerte este gerador antes de rodar")
        return t, False
    if tem:
        parar(f"a biblioteca tem só parte das formas novas: {', '.join(tem)}")
    i = t.index('<sc-for list="{{t.tamanhos}}"', t.index('<sc-for list="{{secoes}}"'))
    f = t.index('<span class="rot">{{z.texto}}</span>', i)
    fim = t.rfind("</sc-if>", i, f) + len("</sc-if>")
    novo = "".join(f'\n<sc-if value="{{{{z.{k}}}}}" hint-placeholder-val="{{{{ false }}}}">{m}</sc-if>' for k, m in NOVAS.items())
    t = t[:fim] + novo + t[fim:]
    t = trocar(t, "a_exportacao: false, cor:",
               "a_exportacao: false, a_disco: false, a_discoG: false, a_linhas: false, a_linhasG: false, cor:")
    t = trocar(t, "exportacao: { a_exportacao: true } };",
               "exportacao: { a_exportacao: true }, disco: { a_disco: true }, linhas: { a_linhas: true } };")
    t = trocar(t, "        eDocG: b.eDoc && s >= 32,\n",
               "        eDocG: b.eDoc && s >= 32,\n"
               "        a_disco: b.a_disco && s < 32,\n        a_discoG: b.a_disco && s >= 32,\n"
               "        a_linhas: b.a_linhas && s < 32,\n        a_linhasG: b.a_linhas && s >= 32,\n")
    t = trocar(t, "      item('exportacao', 'Exportação', 'Preferências > Exportação')\n    ];",
               "      item('exportacao', 'Exportação', 'Preferências > Exportação'),\n"
               "      item('disco', 'Tamanho em disco', 'Visão geral > Tamanho'),\n"
               "      item('linhas', 'Linhas', 'Visão geral > Linhas')\n    ];")
    return t, True


# ---------- prancheta 1: os contadores ----------

# rótulo -> (forma da biblioteca, cor, camadas ligadas, quanto o desenho desce até o pé comum em 44,5)
CONTADORES = [("Tabelas", ("eTab", "#2c6fdc", ()), 4), ("Visão", ("eTab", "#7a52c7", ("Olho",)), 2),
              ("Índices", ("eInd", "#2c6fdc", ()), 2), ("Gatilho", ("eRaio", None, ()), 2),
              ("Tamanho", ("a_disco", None, ()), 0), ("Linhas", ("a_linhas", None, ()), 1)]

# os dois formatos de antes: a faixa original (sem marca) e a de 16 px da primeira rodada (com a marca)
GRADE = ('<div style="display:grid;grid-template-columns:repeat(6,minmax(0,1fr));border:1px solid var(--b2);'
         'border-radius:10px;background:var(--p);overflow:hidden">')
GRADE_16 = GRADE.replace("<div ", f"<div {MARCA} ", 1)
CELULA_ANTIGA = re.compile(r'<div style="padding:12px 16px(?P<borda>;border-left:1px solid var\(--b\))?">'
                           r'<div style="font-size:24px;font-weight:300;line-height:1\.1">(?P<num>(?:(?!</div>).)*)</div>'
                           r'<div class="tit" style="margin-top:4px(?P<flex>;display:flex;align-items:center;gap:6px)?">'
                           r'(?P<ico><span class="sk s16" aria-hidden="true"><svg\b(?:(?!</svg>).)*</svg></span>)?'
                           r'(?P<rot>[^<]+)</div></div>', re.S)
FIM_ANTIGA = "\n</div>\n<div>\n"

# a dock
DOCK = f'<div class="dock" {MARCA}><div class="dock-g">'
FIM_DOCK = "\n</div></div>\n<div>\n"
CELULA_DOCK = re.compile(r'<div class="dk"><span class="dk-ic(?: pe\d)?" aria-hidden="true"><svg\b(?:(?!</svg>).)*</svg></span>'
                         r'<div class="dk-n">(?P<num>(?:(?!</div>).)*)</div><div class="tit">(?P<rot>[^<]+)</div></div>', re.S)

CSS_INI = "/* contadores na dock de 17/10/2026 (icones_resumo.py) */"
CSS_FIM = "/* fim dos contadores na dock */"
CSS = "\n".join([
    CSS_INI,
    # a prateleira: degradê com brilho em cima, fio, luz de 1 px e sombra macia; o contato embaixo de cada ícone
    ".est{--dock1:#2a2b30;--dock2:#212226;--docktopo:rgba(255,255,255,.035);--dockb:rgba(255,255,255,.09);"
    "--dockluz:rgba(255,255,255,.08);--docksomb:0 1px 2px rgba(0,0,0,.35),0 12px 24px -14px rgba(0,0,0,.7);--contato:rgba(0,0,0,.55)}",
    ".est.claro{--dock1:#fbfbfc;--dock2:#eef0f3;--docktopo:rgba(255,255,255,.8);--dockb:rgba(0,0,0,.1);"
    "--dockluz:rgba(255,255,255,.95);--docksomb:0 1px 2px rgba(0,0,0,.08),0 12px 24px -14px rgba(0,0,0,.28);--contato:rgba(0,0,0,.28)}",
    ".dock{container-type:inline-size}",
    ".dock-g{position:relative;display:grid;grid-template-columns:repeat(6,minmax(0,1fr));row-gap:14px}",
    '.dock-g::before,.dock-g::after{content:"";position:absolute;left:0;right:0;border-radius:10px;'
    "background:linear-gradient(to bottom,var(--docktopo),transparent 22px),linear-gradient(to bottom,var(--dock1),var(--dock2));"
    "border:1px solid var(--dockb);box-shadow:inset 0 1px 0 var(--dockluz),var(--docksomb)}",
    ".dock-g::before{top:26px;bottom:0}",
    ".dock-g::after{display:none}",
    "@container (max-width:700px){.dock-g{grid-template-columns:repeat(3,minmax(0,1fr))}"
    ".dock-g::before{bottom:auto;height:82px}.dock-g::after{display:block;top:148px;height:82px}}",
    ".dk{position:relative;z-index:1;height:108px;display:flex;flex-direction:column;align-items:center;min-width:0;padding:0 6px}",
    ".dk-ic{position:relative;width:48px;height:48px;flex:none;--pe:0px}",
    ".dk-ic.pe1{--pe:1px}.dk-ic.pe2{--pe:2px}.dk-ic.pe4{--pe:4px}",
    ".dk-ic svg{display:block;position:relative;transform:translateY(var(--pe));"
    "-webkit-box-reflect:below calc(-7px - 2 * var(--pe)) linear-gradient(to bottom,transparent calc(36.5px - var(--pe)),rgba(255,255,255,.2) calc(44.5px - var(--pe)))}",
    '.dk-ic::before{content:"";position:absolute;left:6px;right:6px;top:40px;height:9px;border-radius:50%;'
    # sem a animação do hover (pedido de 17/10/2026): a dock fica parada
    "background:radial-gradient(closest-side,var(--contato),transparent)}",
    ".dk-n{margin-top:10px;font-size:24px;font-weight:300;line-height:1.1;white-space:nowrap;font-variant-numeric:tabular-nums}",
    ".dk .tit{margin-top:3px;white-space:nowrap}",
    CSS_FIM,
])


def svg_contador(formas, forma, cor, ligadas):
    """O desenho -grande (o de 48 da biblioteca, com o contorno fino) no lado da dock."""
    s = icones_app.resolver(formas[icones_app.GRANDE.get(forma, forma)], ligadas, True)
    if ("{{z.cor}}" in s) != (cor is not None):
        parar(f"a cor da forma {forma} não combina com a do contador")
    if cor:
        s = s.replace("{{z.cor}}", cor)
    return s.replace('width="48" height="48"', f'width="{LADO}" height="{LADO}"', 1)


def ler_contadores(t):
    """(início, fim, [(número, rótulo)]) da faixa, em qualquer um dos três formatos conhecidos."""
    achados = [(c, t.count(c)) for c in (GRADE, GRADE_16, DOCK)]
    if sum(n for _c, n in achados) != 1:
        parar(f"a prancheta 1 tem {sum(n for _c, n in achados)} faixas de contadores (esperava 1)")
    cab = next(c for c, n in achados if n)
    ini = t.index(cab)
    dock = cab == DOCK
    fim = t.index(FIM_DOCK if dock else FIM_ANTIGA, ini)
    corpo = t[ini + len(cab):fim]
    padrao = CELULA_DOCK if dock else CELULA_ANTIGA
    cels = list(padrao.finditer(corpo))
    if padrao.sub("", corpo).strip() or len(cels) != len(CONTADORES):
        parar("os contadores da Visão geral não estão no formato conhecido")
    if not dock:
        marcada = cab == GRADE_16
        for k, m in enumerate(cels):
            if bool(m.group("borda")) != (k > 0):
                parar(f"o contador {m.group('rot')!r} está com a borda fora do lugar")
            if bool(m.group("ico")) != marcada or bool(m.group("flex")) != marcada:
                parar(f"o contador {m.group('rot')!r} não combina com a marca {MARCA}")
    fim += len("\n</div></div>" if dock else "\n</div>")
    return ini, fim, [(m.group("num"), m.group("rot")) for m in cels]


def com_css(t):
    a, b = t.count(CSS_INI), t.count(CSS_FIM)
    if (a, b) == (1, 1):
        i, f = t.index(CSS_INI), t.index(CSS_FIM) + len(CSS_FIM)
        if f < i:
            parar("o CSS da dock está com os comentários trocados")
        return t[:i] + CSS + t[f:]
    if (a, b) != (0, 0):
        parar("o CSS da dock tem só um dos comentários; acerte à mão antes de rodar")
    if t.count("</style>") != 1:
        parar("a prancheta 1 não tem um <style> só")
    return t.replace("</style>", CSS + "\n</style>", 1)


def contadores(t, formas):
    ini, fim, valores = ler_contadores(t)
    novas = []
    for k, ((num, rot_lido), (rot, ic, pe)) in enumerate(zip(valores, CONTADORES)):
        if rot_lido != rot:
            parar(f"o contador {k + 1} é {rot_lido!r}, e não {rot!r}")
        cls = f"dk-ic pe{pe}" if pe else "dk-ic"
        novas.append(f'<div class="dk"><span class="{cls}" aria-hidden="true">{svg_contador(formas, *ic)}</span>'
                     f'<div class="dk-n">{num}</div><div class="tit">{rot}</div></div>')
    t = t[:ini] + DOCK + "\n" + "\n".join(novas) + "\n</div></div>" + t[fim:]
    t = com_css(t)
    # toda url(#…) dos ícones precisa existir na prancheta
    ids = set(re.findall(r'\bid="([^"]+)"', t))
    for u in sorted({u for c in novas for u in re.findall(r"url\(#([^)]+)\)", c)}):
        if u not in ids:
            parar(f"a prancheta 1 não define #{u}")
    return t


def main():
    feitas = 0
    cam_ic = PASTA / "Icones.dc.html"
    ic = cam_ic.read_text(encoding="utf-8")
    ic2, mudou = biblioteca(ic)
    formas = icones_app.moldes(ic2)
    cam_main = PASTA / "Main.dc.html"
    main_t = cam_main.read_text(encoding="utf-8")
    main2 = contadores(main_t, formas)
    if mudou:
        cam_ic.write_text(ic2, encoding="utf-8", newline="\n")
        print("Icones.dc.html: disco e linhas na seção Aplicativo")
        feitas += 1
    else:
        print("Icones.dc.html já tem disco e linhas")
    if main2 != main_t:
        cam_main.write_text(main2, encoding="utf-8", newline="\n")
        print("Main.dc.html: os seis contadores da Visão geral na dock")
        feitas += 1
    else:
        print("Main.dc.html já tem a dock dos contadores")
    print(feitas, "pranchetas alteradas")


if __name__ == "__main__":
    main()
