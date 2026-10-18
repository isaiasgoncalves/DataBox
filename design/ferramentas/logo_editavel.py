"""Cópia editável do logo: cada peça vira uma camada solta (div com posição absoluta e o SVG só daquela
peça), na mesma ordem de desenho, para o Isaías arrastar, girar e redimensionar no canvas."""
import re
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
sys.path.insert(0, str(AQUI))
import logo  # noqa: E402  (gerador do logo, nesta mesma pasta)

PROJETO = AQUI.parent / "canvas"
K = 1.3  # escala do palco: 512 unidades viram 665,6 px

SOMBRA_PECA = ('<filter id="lgSombraPeca" x="-20%" y="-20%" width="140%" height="145%"><feDropShadow dx="0" dy="5" stdDeviation="5" '
               'style="flood-color:#000000;flood-opacity:.35"></feDropShadow></filter>')
COM_SOMBRA = {"abas de trás": "lgSombra", "frente da caixa": "lgSombra", "gráfico": "lgSombraPeca", "lápis": "lgSombraPeca",
              "planilha": "lgSombraPeca", "banco": "lgSombraPeca", "lupa": "lgSombraPeca"}


def caixa_limite(svg):
    xs, ys = [], []

    def pt(x, y, folga=0.0):
        xs.extend([x - folga, x + folga])
        ys.extend([y - folga, y + folga])

    for d in re.findall(r' d="([^"]+)"', svg):
        toks = re.findall(r"[MLAZHV]|-?\d*\.?\d+(?:e-?\d+)?", d)
        i, cmd, ult = 0, None, (0.0, 0.0)
        while i < len(toks):
            t = toks[i]
            if t in "MLAZHV":
                cmd = t
                i += 1
                continue
            if cmd in ("M", "L"):
                ult = (float(toks[i]), float(toks[i + 1]))
                pt(*ult)
                i += 2
            elif cmd == "A":
                rx, ry = float(toks[i]), float(toks[i + 1])
                x, y = float(toks[i + 5]), float(toks[i + 6])
                meio = ((ult[0] + x) / 2, (ult[1] + y) / 2)
                pt(x, y)
                pt(meio[0], meio[1] - ry)
                pt(meio[0], meio[1] + ry)
                ult = (x, y)
                i += 7
            elif cmd == "H":
                ult = (float(t), ult[1])
                pt(*ult)
                i += 1
            elif cmd == "V":
                ult = (ult[0], float(t))
                pt(*ult)
                i += 1
            else:
                i += 1
    for cx, cy, r in re.findall(r'<circle[^>]*?cx="([-\d.]+)" cy="([-\d.]+)" r="([\d.]+)"', svg):
        pt(float(cx), float(cy), float(r))
    for cx, cy, rx, ry in re.findall(r'<ellipse[^>]*?cx="([-\d.]+)" cy="([-\d.]+)" rx="([\d.]+)" ry="([\d.]+)"', svg):
        pt(float(cx), float(cy), max(float(rx), float(ry)))
    pad = 6
    return min(xs) - pad, min(ys) - pad, max(xs) - min(xs) + 2 * pad, max(ys) - min(ys) + 2 * pad


def camadas():
    bruto = logo.gerar(False, bruto=True)
    # sem o grupo que punha uma sombra só no conjunto (cada camada leva a sua) e só com a luz âmbar
    bruto = [x for x in bruto if x != '<g filter="url(#lgSombra)">']
    assert bruto[-1] == "</g>"
    bruto = bruto[:-1]
    bruto = [x for x in bruto if 'class="luzA"' not in x]
    bruto = [x.replace(' class="luzB"', "") for x in bruto]
    grupos, nome = [], None
    for x in bruto:
        if x.startswith("@@camada:"):
            nome = x.split(":", 1)[1]
            grupos.append([nome, []])
        else:
            grupos[-1][1].append(x)
    return [(n, "\n".join(c)) for n, c in grupos]


def prancheta():
    pecas = []
    for nome, svg in camadas():
        x, y, w, h = caixa_limite(svg)
        filtro = COM_SOMBRA.get(nome)
        corpo = f'<g filter="url(#{filtro})">{svg}</g>' if filtro else svg
        pecas.append(
            f'<div class="camada" title="{nome}" aria-label="{nome}" style="position:absolute;left:{x * K:.1f}px;top:{y * K:.1f}px;width:{w * K:.1f}px;height:{h * K:.1f}px">'
            f'<svg viewBox="{x:.1f} {y:.1f} {w:.1f} {h:.1f}" width="100%" height="100%" aria-hidden="true" style="display:block;overflow:visible">{corpo}</svg></div>')
    lista = "".join(f"<li>{n}</li>" for n, _ in reversed(camadas()))
    estilo = """body{margin:0;background:#0e0f11}
.est{--fundo0:#111214;--painel:#1e1f22;--b2:rgba(255,255,255,.12);--t:#ececee;--t2:#b9bbc0;--t3:#9597a0;--vidrob:rgba(255,255,255,.09);--vidroluz:rgba(255,255,255,.06);--bt1:#3b3c40;--bt2:#26272a;--btb:rgba(0,0,0,.6);--btluz:rgba(255,255,255,.13);--textura:repeating-linear-gradient(135deg,rgba(255,255,255,.022) 0 1px,transparent 1px 3px);font-family:"Segoe UI Variable Text","Segoe UI",system-ui,sans-serif;font-size:12px;line-height:1.4;color:var(--t);background:var(--fundo0)}
.est *{box-sizing:border-box}
.painel{background:var(--painel);border:1px solid var(--vidrob);border-radius:14px;box-shadow:inset 0 1px 0 var(--vidroluz),0 1px 3px rgba(0,0,0,.2);overflow:hidden;display:flex;flex-direction:column}
.cab{height:38px;flex:none;display:flex;align-items:center;gap:8px;padding:0 14px;border-bottom:1px solid var(--btb);background:var(--textura),linear-gradient(to bottom,var(--bt1),var(--bt2));box-shadow:inset 0 1px 0 var(--btluz)}
.tit{font-size:11px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;color:var(--t3)}
.palco{position:relative;width:665.6px;height:665.6px;flex:none}
.camada{outline:1px dashed transparent}
.camada:hover{outline-color:rgba(255,255,255,.25)}
ol{margin:0;padding-left:20px;color:var(--t2);display:flex;flex-direction:column;gap:4px}"""
    return f"""<!doctype html>
<html lang="pt-BR">
<head>
<meta charset="utf-8">
<title>Logo editável</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<style>
{estilo}
</style>
</helmet>
<div class="est" style="width:100%;min-height:900px;display:flex;gap:16px;padding:24px 28px 28px">
<svg width="0" height="0" style="position:absolute" aria-hidden="true" focusable="false"><defs>
{logo.DEFS}
{SOMBRA_PECA}
</defs></svg>
<section class="painel" style="flex:none" aria-label="Logo em camadas">
<div class="cab"><span class="tit">Logo em camadas · arraste, gire ou redimensione cada peça</span></div>
<div style="padding:28px;display:flex;align-items:center;justify-content:center;background:#141518">
<div class="palco">
{chr(10).join(pecas)}
</div>
</div>
</section>
<section class="painel" style="flex:1;min-width:300px" aria-label="Camadas">
<div class="cab"><span class="tit">Camadas</span></div>
<div style="padding:16px 18px;display:flex;flex-direction:column;gap:14px">
<p style="margin:0;color:var(--t2)">De cima para baixo, na ordem em que aparecem. As peças que saem da caixa ficam entre o interior e a frente, então continuam escondidas pela frente da caixa quando descem. Quando ficar do seu jeito, me avise que eu levo as posições para o logo oficial e para as cópias nas telas.</p>
<ol>{lista}</ol>
</div>
</section>
</div>
</x-dc>
<script type="text/x-dc" data-dc-script data-props='{{"$preview":{{"width":1440,"height":900}}}}'>
class Component extends DCLogic {{
  renderVals() {{
    return {{}};
  }}
}}
</script>
</body>
</html>
"""


if __name__ == "__main__":
    destino = PROJETO / "LogoEditavel.dc.html"
    destino.write_text(prancheta(), encoding="utf-8", newline="\n")
    for nome, svg in camadas():
        x, y, w, h = caixa_limite(svg)
        print(f"{nome:16s} x {x:6.1f} y {y:6.1f} w {w:6.1f} h {h:6.1f}", file=sys.stderr)
    print(destino, destino.stat().st_size, "bytes", file=sys.stderr)
