"""Logo editável: reordena as camadas sem mexer nas posições que o Isaías ajustou.
O canvas aninhou algumas camadas (lápis dentro do banco; lupa e gráfico dentro da frente da caixa), então cada
camada volta a ser filha direta do palco, com left/top somados aos dos pais, e entra na ordem pedida."""
import re
import sys
from pathlib import Path

ARQ = Path(__file__).resolve().parent.parent / "canvas" / "LogoEditavel.dc.html"
sys.stdout.reconfigure(encoding="utf-8")

# de trás para a frente: o gráfico no fundo, logo depois do interior; a lupa antes da frente, para continuar dentro
ORDEM = ["luz", "abas de trás", "raios", "interior da caixa", "gráfico", "planilha", "banco", "lápis", "lupa",
         "frente da caixa"]

texto = ARQ.read_text(encoding="utf-8")
ini = texto.index('<div class="palco">') + len('<div class="palco">')

# percorre <div ...> e </div> a partir do palco, guardando a pilha de camadas abertas
camadas, pilha, i = {}, [], ini
for m in re.compile(r'<div class="camada"([^>]*)>|</div>').finditer(texto, ini):
    if m.group(0) == "</div>":
        if not pilha:
            fim_palco = m.start()  # o </div> que fecha o palco
            break
        pilha.pop()
        continue
    attrs = m.group(1)
    nome = re.search(r'title="([^"]+)"', attrs).group(1)
    estilo = re.search(r'style="([^"]*)"', attrs).group(1)
    props = {}
    for decl in estilo.split(";"):
        if decl.strip():
            k, v = decl.split(":", 1)
            props[k.strip()] = v.strip()
    fim_svg = texto.index("</svg>", m.end()) + len("</svg>")
    svg = texto[m.end():fim_svg]
    assert svg.startswith("<svg") and "<div" not in svg, nome
    pais = list(pilha)
    for p in pais:
        assert "transform" not in camadas[p]["props"], f"pai com transformação: {p}"
    camadas[nome] = {"props": props, "svg": svg, "pais": pais}
    pilha.append(nome)
else:
    raise SystemExit("não achei o fim do palco")

assert sorted(camadas) == sorted(ORDEM), sorted(camadas)


def px(v):
    assert v.endswith("px"), v
    return float(v[:-2])


linhas = []
for nome in ORDEM:
    c = camadas[nome]
    props = dict(c["props"])
    x = px(props["left"]) + sum(px(camadas[p]["props"]["left"]) for p in c["pais"])
    y = px(props["top"]) + sum(px(camadas[p]["props"]["top"]) for p in c["pais"])
    props["left"], props["top"] = f"{x:g}px", f"{y:g}px"
    estilo = ";".join(f"{k}:{v}" for k, v in props.items())
    linhas.append(f'<div class="camada" title="{nome}" aria-label="{nome}" style="{estilo}">{c["svg"]}</div>')
    print(f"{nome:18s} left {x:7.1f} top {y:7.1f}" + (f"  (estava dentro de: {' > '.join(c['pais'])})" if c["pais"] else ""))

texto = texto[:ini] + "\n" + "\n".join(linhas) + "\n" + texto[fim_palco:]
lista = "".join(f"<li>{n}</li>" for n in reversed(ORDEM))
texto, n = re.subn(r"<ol>.*?</ol>", f"<ol>{lista}</ol>", texto, count=1, flags=re.S)
assert n == 1
ARQ.write_text(texto, encoding="utf-8", newline="\n")
print("LogoEditavel.dc.html reordenado")
