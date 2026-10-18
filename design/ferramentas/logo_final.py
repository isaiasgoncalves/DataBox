"""Logo final: monta o logo oficial a partir das camadas que o Isaías posicionou no logo editável (prancheta 11).

Cada camada é um div com left/top/width/height (px, palco com escala K = 1,3), um transform de CSS opcional
(girado/esticado em torno do centro do div) e um SVG com viewBox em unidades do logo (preserveAspectRatio
padrão: escala uniforme, centralizado). Aqui cada camada vira um <g transform> no viewBox 0 0 512 512, com
exatamente o que o navegador desenha. Depois:
- Logo.dc.html é refeito (logo.prancheta) com o logo final no lugar do gerado pelo logo.py;
- o logo grande do Início (60 px) troca pelo final;
- logo/fragmentos recebe final.svgfrag e logo/ o SVG avulso databox.svg. A versão simplificada (databox-simples.svg) deixou
  de ser usada em 17/10/2026 (só o logo completo, em todos os tamanhos): ela só continua sendo gravada, como registro,
  e a prancheta 10 e o app não a usam."""
import math
import re
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
sys.path.insert(0, str(AQUI))
import logo  # noqa: E402
import logo_editavel  # noqa: E402

PROJETO = logo.PROJETO
SAIDA = AQUI.parent / "logo" / "fragmentos"
K = logo_editavel.K
sys.stdout.reconfigure(encoding="utf-8")

CAMADA = re.compile(r'<div class="camada"([^>]*)>(<svg [^>]*>)(.*?)</svg></div>', re.S)
FUNCAO = re.compile(r"(rotate|scaleX|scaleY|scale|translateX|translateY|translate)\(([^)]*)\)")


def num(v):
    return float(re.sub(r"(px|deg)$", "", v.strip()))


def svg_de_css(transform):
    """transform de CSS → lista de transformações de SVG (mesma ordem; px viram unidades do logo)."""
    partes = []
    for nome, args in FUNCAO.findall(transform or ""):
        a = [num(x) for x in args.split(",")]
        if nome == "rotate":
            partes.append(f"rotate({a[0]:g})")
        elif nome == "scale":
            partes.append(f"scale({a[0]:g} {a[1] if len(a) > 1 else a[0]:g})")
        elif nome == "scaleX":
            partes.append(f"scale({a[0]:g} 1)")
        elif nome == "scaleY":
            partes.append(f"scale(1 {a[0]:g})")
        elif nome == "translate":
            partes.append(f"translate({a[0] / K:g} {(a[1] if len(a) > 1 else 0) / K:g})")
        elif nome == "translateX":
            partes.append(f"translate({a[0] / K:g} 0)")
        elif nome == "translateY":
            partes.append(f"translate(0 {a[0] / K:g})")
    resto = FUNCAO.sub("", transform or "").strip()
    assert not resto, f"transform não suportado: {transform!r}"
    return partes


def matriz(partes):
    """Matriz 2×3 (a, b, c, d, e, f) das transformações de SVG, para medir os cantos."""
    m = (1, 0, 0, 1, 0, 0)

    def mult(p, q):
        a, b, c, d, e, f = p
        A, B, C, D, E, F = q
        return (a * A + c * B, b * A + d * B, a * C + c * D, b * C + d * D, a * E + c * F + e, b * E + d * F + f)

    for t in partes:
        nome, args = re.match(r"(\w+)\(([^)]*)\)", t).groups()
        v = [float(x) for x in args.replace(",", " ").split()]
        if nome == "translate":
            q = (1, 0, 0, 1, v[0], v[1] if len(v) > 1 else 0)
        elif nome == "scale":
            q = (v[0], 0, 0, v[1] if len(v) > 1 else v[0], 0, 0)
        elif nome == "rotate":
            r = math.radians(v[0])
            q = (math.cos(r), math.sin(r), -math.sin(r), math.cos(r), 0, 0)
        else:
            raise ValueError(t)
        m = mult(m, q)
    return m


def camadas_finais():
    texto = (PROJETO / "LogoEditavel.dc.html").read_text(encoding="utf-8")
    ini = texto.index('<div class="palco">')
    fim = texto.index("</section>", ini)
    palco = texto[ini:fim]
    assert palco.count('<div class="camada"') == len(CAMADA.findall(palco)), "camadas aninhadas: rode reordenar_logo.py"
    grupos, caixa = [], [math.inf, math.inf, -math.inf, -math.inf]
    for attrs, abre, corpo in CAMADA.findall(palco):
        nome = re.search(r'title="([^"]+)"', attrs).group(1)
        props = {}
        for decl in re.search(r'style="([^"]*)"', attrs).group(1).split(";"):
            if decl.strip():
                k, v = decl.split(":", 1)
                props[k.strip()] = v.strip()
        L, T, W, H = (num(props[k]) for k in ("left", "top", "width", "height"))
        vx, vy, vw, vh = (float(x) for x in re.search(r'viewBox="([^"]+)"', abre).group(1).split())
        s = min(W / vw, H / vh)
        ox, oy = (L + (W - vw * s) / 2) / K, (T + (H - vh * s) / 2) / K
        cx, cy = (L + W / 2) / K, (T + H / 2) / K
        css = svg_de_css(props.get("transform"))
        partes = []
        if css:
            partes += [f"translate({cx:.2f} {cy:.2f})", *css, f"translate({-cx:.2f} {-cy:.2f})"]
        # o palco guarda px com uma casa: diferença de menos de 0,1 unidade ou 0,2% de escala é arredondamento
        if abs(s / K - 1) > .002:
            partes += [f"translate({ox:.2f} {oy:.2f})", f"scale({s / K:.4f})", f"translate({-vx:g} {-vy:g})"]
        elif abs(ox - vx) > .1 or abs(oy - vy) > .1:
            partes.append(f"translate({ox - vx:.2f} {oy - vy:.2f})")
        # caixa do conjunto: os cantos do retângulo da camada, já transformados
        m = matriz(partes + [f"translate({vx:g} {vy:g})"])
        for px, py in ((0, 0), (vw, 0), (0, vh), (vw, vh)):
            X, Y = m[0] * px + m[2] * py + m[4], m[1] * px + m[3] * py + m[5]
            caixa = [min(caixa[0], X), min(caixa[1], Y), max(caixa[2], X), max(caixa[3], Y)]
        g = f'<g transform="{" ".join(partes)}">' + corpo + "</g>" if partes else corpo
        grupos.append(f"<!-- {nome} -->\n{g}")
        print(f"{nome:18s} {' '.join(partes) or '(como gerado)'}")
    print("caixa do conjunto (com folga das camadas):", [round(v, 1) for v in caixa])
    return "\n".join(grupos)


def so_ambar(linhas):
    """Fragmento bruto do logo.py → só a luz âmbar, sem classes nem marcadores de camada."""
    linhas = [x for x in linhas if not x.startswith("@@camada:") and 'class="luzA"' not in x]
    return "\n".join(x.replace(' class="luzB"', "") for x in linhas)


DEFS_AMBAR = "\n".join(x for x in logo.DEFS.split("\n") if not re.search(r'id="lg(LuzA|DentroA|RaioA)"', x))
DEFS_FINAIS = DEFS_AMBAR + "\n" + logo_editavel.SOMBRA_PECA


def avulso(frag, titulo):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512" role="img">\n'
            f"<title>{titulo}</title>\n<defs>\n{DEFS_FINAIS}\n</defs>\n{frag}\n</svg>\n")


if __name__ == "__main__":
    final = camadas_finais()
    simples = so_ambar(logo.gerar(True, bruto=True))
    SAIDA.mkdir(exist_ok=True)
    (SAIDA / "final.svgfrag").write_text(final, encoding="utf-8")
    (SAIDA.parent / "databox.svg").write_text(avulso(final, "DataBox"), encoding="utf-8", newline="\n")
    (SAIDA.parent / "databox-simples.svg").write_text(avulso(simples, "DataBox"), encoding="utf-8", newline="\n")

    # prancheta 10 com o logo final
    logo.DEFS = logo.DEFS + "\n" + logo_editavel.SOMBRA_PECA
    pagina = logo.prancheta(final)
    antigo = ("Caixa de papelão aberta, em isométrico, com um banco e uma planilha saindo dela, uma lupa na frente e "
              "um lápis no fundo, e luz âmbar por trás.")
    novo = ("Caixa de papelão aberta, em isométrico, com as ferramentas saindo dela, mas ainda dentro: o banco e o "
            "lápis inclinados, a planilha, o gráfico de barras ao fundo e a lupa, com luz âmbar por trás.")
    assert pagina.count(antigo) == 1
    pagina = pagina.replace(antigo, novo)
    (PROJETO / "Logo.dc.html").write_text(pagina, encoding="utf-8", newline="\n")

    # Início: o logo grande troca pelo final (o da barra de título é de logo_so_completo.py)
    inicio = PROJETO / "Inicio.dc.html"
    texto = inicio.read_text(encoding="utf-8")
    grande = re.compile(r'(<svg class="logo" viewBox="0 0 512 512" width="60" height="60" aria-hidden="true">).*?(</svg>)', re.S)
    texto, n = grande.subn(lambda m: m.group(1) + final + m.group(2), texto)
    assert n == 1, n
    if 'id="lgSombraPeca"' not in texto:
        fim_sombra = texto.index('<filter id="lgSombra"')
        fim_sombra = texto.index("</filter>", fim_sombra) + len("</filter>")
        texto = texto[:fim_sombra] + "\n" + logo_editavel.SOMBRA_PECA + texto[fim_sombra:]
    inicio.write_text(texto, encoding="utf-8", newline="\n")
    print("Logo.dc.html e Inicio.dc.html com o logo final; logo/databox.svg e logo/databox-simples.svg gravados")
