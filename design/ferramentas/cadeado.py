"""Estado "Bloqueado": cadeado de frente, refeito do zero no padrão sólido dos outros ícones de estado.
Corpo de latão (icAmbar, realce em cima, sombra embaixo, contorno .35, sombra curta) e arco de aço tubular,
sombreado em camadas concêntricas (borda escura, miolo claro) para parecer cilíndrico em qualquer tamanho.
Com --previa, só grava previa_cadeado.html (o cadeado em vários tamanhos ao lado dos vizinhos) para conferir."""
import re
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
PASTA = AQUI.parent / "canvas"
sys.stdout.reconfigure(encoding="utf-8")

# proporções: corpo 28 x 21 (1,33 : 1), arco com vão de ~68% da largura e ~70% da altura do corpo acima dele
CORPO = dict(x=10.0, y=21.5, w=28.0, h=21.0, r=4.6)
ARCO = "M16.75 26V16.6a7.25 7.25 0 0 1 14.5 0V26"


def _tubo(passos=9):
    """(largura, cor, opacidade) da borda escura para o brilho no meio do tubo, em passos curtos para não formar faixas."""
    a, b = (0x5c, 0x65, 0x73), (0xff, 0xff, 0xff)
    camadas = [(6.2, "#000000", .42)]
    for i in range(passos):
        t = (i / (passos - 1)) ** 1.25
        cor = "#" + "".join(f"{round(p + (q - p) * t):02x}" for p, q in zip(a, b))
        camadas.append((round(4.8 - i * (4.8 - .6) / (passos - 1), 2), cor, 1))
    return camadas


TUBO = _tubo()
FECHADURA = ('<circle cx="24" cy="30.4" r="2.6"></circle>'
             '<path d="M22.7 31.6h2.6l.8 5.4a.6.6 0 0 1-.6.7h-3a.6.6 0 0 1-.6-.7z"></path>')


def cadeado():
    x, y, w, h, r = CORPO["x"], CORPO["y"], CORPO["w"], CORPO["h"], CORPO["r"]
    partes = [f'<path d="{ARCO}" fill="none" stroke="{cor}" style="stroke-opacity:{op};stroke-width:{larg}"></path>'
              for larg, cor, op in TUBO]
    partes += [
        f'<rect filter="url(#icSombra)" x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="url(#icAmbar)" stroke="#000000" style="stroke-opacity:.35"></rect>',
        f'<rect x="{x + .6}" y="{y + .6}" width="{w - 1.2}" height="8.4" rx="{r - .6}" fill="url(#icRealce)"></rect>',
        f'<rect x="{x}" y="{y + h - 10}" width="{w}" height="10" rx="{r}" fill="url(#icFundo)"></rect>',
        # chanfro: filete claro por dentro da borda, como uma placa de latão estampada
        f'<rect x="{x + 1.5}" y="{y + 1.5}" width="{w - 3}" height="{h - 3}" rx="{r - 1.4}" fill="none" stroke="#ffffff" style="stroke-opacity:.3;stroke-width:.9"></rect>',
        # fechadura gravada: o recorte escuro com um filete claro logo abaixo
        f'<g fill="#ffffff" style="fill-opacity:.45" transform="translate(0 .8)">{FECHADURA}</g>',
        f'<g fill="#3d2a00">{FECHADURA}</g>',
    ]
    return "".join(partes)


BLOCO = re.compile(r'(<sc-if value="\{\{z\.st_bloqueado\}\}"[^>]*><svg [^>]*>).*?(</svg></sc-if>)', re.S)
DEFS_VELHOS = re.compile(r'<linearGradient id="icCad(?:Topo|Lado)".*?</linearGradient>\n')

caminho = PASTA / "Icones.dc.html"
texto = caminho.read_text(encoding="utf-8")

if "--previa" in sys.argv:
    defs = re.search(r"<defs>(.*?)</defs>", texto, re.S).group(1)
    vizinhos = {n: re.search(r'<sc-if value="\{\{z\.' + n + r'\}\}"[^>]*>(<svg .*?</svg>)</sc-if>', texto, re.S).group(1)
                for n in ("st_atencao", "st_erro", "st_ok", "st_bloqueado")}

    def svg(corpo, s):
        return f'<svg viewBox="0 0 48 48" width="{s}" height="{s}">{corpo}</svg>'

    def interno(m):
        return re.search(r"<svg [^>]*>(.*)</svg>", m, re.S).group(1)

    linhas = []
    for fundo in ("#1b1c1f", "#f3f3f5"):
        celulas = []
        for nome, corpo in [("novo", cadeado()), ("atual", interno(vizinhos["st_bloqueado"])),
                            ("atenção", interno(vizinhos["st_atencao"])), ("erro", interno(vizinhos["st_erro"])),
                            ("ok", interno(vizinhos["st_ok"]))]:
            celulas.append('<div class="c">' + "".join(svg(corpo, s) for s in (144, 48, 24, 16)) + f"<b>{nome}</b></div>")
        linhas.append(f'<div class="l" style="background:{fundo}">{"".join(celulas)}</div>')
    previa = AQUI / "previa_cadeado.html"
    previa.write_text(
        '<!doctype html><meta charset="utf-8"><style>body{margin:0;font:12px Segoe UI;color:#888}'
        '.l{display:flex;gap:28px;padding:20px}.c{display:flex;align-items:end;gap:14px}b{align-self:start}</style>'
        f'<svg width="0" height="0" style="position:absolute"><defs>{defs}</defs></svg>' + "".join(linhas),
        encoding="utf-8")
    print(previa)
    sys.exit(0)

texto, n = BLOCO.subn(lambda m: m.group(1) + cadeado() + m.group(2), texto)
assert n == 1, n
texto = DEFS_VELHOS.sub("", texto)
assert "icCad" not in texto
for ref in ("icAmbar", "icRealce", "icFundo", "icSombra"):
    assert f'id="{ref}"' in texto, ref
caminho.write_text(texto, encoding="utf-8", newline="\n")
print("Icones.dc.html: cadeado refeito")
