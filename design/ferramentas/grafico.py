"""A prancheta dos gráficos (`FuturoGrafico.dc.html`, PR 7 do plano da 0.3), refeita em 26/10/2026 sobre o plano.

A de antes (um "Tabela · Gráfico" no resultado da Consulta, feita à mão em 13/10/2026) saiu: as decisões 6 e 7 da 0.3
puseram o gráfico numa vista "Gráfico" das Propriedades, com a prévia pequena, e o "Abrir em aba" para o grande. Cenas,
cada uma com título e legenda, como nas pranchetas 15 a 17:
- (a) a vista Gráfico nas Propriedades de clientes: o tipo, as colunas, a prévia e as ações;
- (b) o gráfico em aba, com a dica do ponto e o rodapé com a posição do cursor;
- (c a f) os quatro tipos: barras, linhas (do resultado de uma consulta), histograma e dispersão;
- (g) vazio, (h) carregando e (i) a amostra da dispersão acima de 100 mil pontos;
- (j) Copiar a imagem, Salvar como PNG e Abrir no Python, com o menu do gráfico e os avisos;
- (k) o texto 40% mais longo do pseudoidioma da P7 ao lado do português.
Os sólidos dos tipos (g_barras, g_linhas, g_histograma, g_dispersao) e o da vista (g_colunas) já estão na biblioteca
(prancheta 9); nenhum ícone novo. Os gráficos são SVG desenhados aqui, com as cores dos tokens.
Refaz a prancheta inteira; recusa gravar por cima de um FuturoGrafico.dc.html sem a marca dele, a não ser a de
13/10/2026, que ele substitui (`--substituir`). Guarda o lugar no canvas.json e acerta a altura.
Uso: python -I -X utf8 grafico.py [--substituir]"""
import math
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pecas as p  # noqa: E402

MARCA = "/* prancheta dos gráficos, PR 7 da 0.3 (grafico.py) */"
ARQUIVO = "FuturoGrafico.dc.html"
ALTURA = 5822
TITULO = "Gráficos: a vista Gráfico nas Propriedades e o gráfico em aba (0.3, PR 7)"

PROPRIO = """
.vg{padding:2px 12px 12px;display:flex;flex-direction:column;gap:10px}
.vg .lc{display:flex;flex-direction:column;gap:4px;min-width:0}
.vg .lc>.r{display:flex;align-items:baseline;gap:8px;color:var(--t3)}
.vg .lc>.r .v{margin-left:auto;color:var(--t2);text-align:right}
.vg .duas{display:grid;grid-template-columns:repeat(auto-fit,minmax(120px,1fr));gap:10px}
.vg .campo{width:100%;min-width:0}
.vg .campo .cv{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.vg .chips{display:flex;flex-wrap:wrap;gap:6px}
.chip-c{display:inline-flex;align-items:center;gap:6px;height:26px;padding:0 4px 0 8px;border-radius:999px;border:1px solid var(--fb);background:var(--f);white-space:nowrap}
.chip-c .cor{width:8px;height:8px;border-radius:50%;flex:none}
.chip-c .x{width:18px;height:18px;border:0;border-radius:50%;background:transparent;color:var(--t3);display:inline-flex;align-items:center;justify-content:center;padding:0}
.seg-ic{display:grid;grid-auto-flow:column;grid-auto-columns:1fr;width:100%}
.seg-ic button{height:30px}
.segv .seg-ic button{height:28px}
.segv .seg-ic button.on{gap:6px}
.previa{position:relative;border:1px solid var(--fb);border-radius:10px;background:var(--c);overflow:hidden;cursor:zoom-in}
.previa .pe{position:absolute;left:10px;bottom:7px;font-size:11px;color:var(--t3)}
.acoes-g{display:flex;flex-wrap:wrap;gap:6px}
.acoes-g .btn{flex:1 1 auto;justify-content:center}
.acoes-g .btn.inteiro{flex-basis:100%}
.grafico{display:block;font-family:inherit;font-variant-numeric:tabular-nums}
.grafico text{font-size:11px;fill:var(--t3)}
.grafico .tit{font-size:13px;font-weight:600;fill:var(--t);letter-spacing:0;text-transform:none}
.grafico .sub{font-size:11px;fill:var(--t3)}
.grafico .eixo{font-size:11px;fill:var(--t2)}
.grafico .rot{font-size:11.5px;fill:var(--t2)}
.grafico .val{font-size:11px;fill:var(--t2)}
.grafico .grade-l{stroke:var(--b);stroke-width:1}
.grafico .base{stroke:var(--b2);stroke-width:1}
.grafico .guia{stroke:var(--t3);stroke-width:1;stroke-dasharray:3 3}
.est{--s1:var(--acc);--s2:#2aa198;--s3:#e0913a}
.est.claro{--s2:#1d8181;--s3:#b4600f}
.dica-g{position:absolute;padding:7px 11px;border-radius:8px;font-size:11.5px;line-height:17px;color:var(--t);white-space:nowrap;pointer-events:none}
.dica-g strong{display:block;font-weight:600}
.dica-g .l{display:flex;align-items:center;gap:8px;color:var(--t2)}
.dica-g .l b{margin-left:auto;padding-left:14px;font-weight:600;color:var(--t);font-variant-numeric:tabular-nums}
.dica-g .cor{width:8px;height:8px;border-radius:50%}
.doc-g{flex:1;min-height:0;position:relative;background:var(--c);display:flex;align-items:stretch}
.rodape .dir{margin-left:auto;color:var(--t3)}
.amostra{display:inline-flex;align-items:center;gap:6px;height:22px;padding:0 10px 0 6px;border-radius:999px;background:color-mix(in oklab,var(--aviso) 14%,transparent);border:1px solid color-mix(in oklab,var(--aviso) 35%,transparent);color:var(--t);font-size:11px;font-weight:600;white-space:nowrap}
.carrega{position:absolute;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:12px;color:var(--t2);text-align:center;padding:16px}
.carrega .ic.gira{width:22px;height:22px;color:var(--acct)}
.carrega .barra{width:220px}
.esqueleto{position:absolute;inset:0;opacity:.35}
.vazio-g{display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px;padding:18px 16px;text-align:center;color:var(--t2);border:1px dashed var(--fb);border-radius:10px;background:var(--p);text-wrap:pretty}
.vazio-g .t3{color:var(--t3);font-size:11.5px}
.par{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:16px;align-items:start}
.doca-p{width:340px;flex:none;border-radius:10px;display:flex;flex-direction:column;overflow:hidden}
.doca-p .segv{padding:8px 10px 6px}
.cartao-g{display:flex;flex-direction:column;gap:10px}
.cartao-g .campos{display:flex;flex-wrap:wrap;gap:10px 14px;align-items:flex-end}
.cartao-g .campos .lc{display:flex;flex-direction:column;gap:4px;color:var(--t3);min-width:150px;flex:1 1 150px}
.cartao-g .campos .campo{width:100%}
.cartao-g .doc{flex:none}
.legenda-s{display:flex;flex-wrap:wrap;gap:14px;color:var(--t2);font-size:11.5px}
.legenda-s span{display:inline-flex;align-items:center;gap:6px}
.legenda-s i{width:14px;height:3px;border-radius:2px;display:inline-block}
.menu-g{position:absolute;width:auto;min-width:230px}
.pilha{display:flex;flex-direction:column;gap:10px;align-items:flex-start}
.pilha .toast{align-self:flex-start}
"""

S = '<span class="sep"></span>'


def ic(chave, n=16):
    return p.sk(p.forma(chave, n), n)


TIPOS = [("barras", "Barras", "g_barras"), ("linhas", "Linhas", "g_linhas"), ("histograma", "Histograma", "g_histograma"),
         ("dispersao", "Dispersão", "g_dispersao")]

# --- os textos, em português e no pseudoidioma da P7 ---------------------------------------------------------------------
PT = {
    "Resumo": "Resumo", "Colunas": "Colunas", "Perfil": "Perfil", "Índices": "Índices", "SQL": "SQL", "Gráfico": "Gráfico",
    "Tipo": "Tipo", "Barras": "Barras", "Linhas": "Linhas", "Histograma": "Histograma", "Dispersão": "Dispersão",
    "Categorias": "Categorias", "Valores": "Valores", "Agregar": "Agregar", "Ordem": "Ordem", "Soma": "Soma",
    "Maior primeiro": "Maior primeiro", "Prévia": "Prévia", "Abrir em aba": "Abrir em aba", "Copiar a imagem": "Copiar a imagem",
    "Salvar como PNG": "Salvar como PNG", "Abrir no Python": "Abrir no Python", "Propriedades": "Propriedades",
    "9 barras · 57 de 60 linhas": "9 barras · 57 de 60 linhas", "Restaurar o zoom": "Restaurar o zoom",
    "Imagem copiada · 1.280 × 720": "Imagem copiada · 1.280 × 720",
}


def pseudo(t):
    """O pseudoidioma da P7: vogais acentuadas, 40% mais longo com ·, entre colchetes ("[Ábrír pástá·····]").
    Os números e os nomes que vêm dos dados não mudam."""
    troca = str.maketrans("aeiouAEIOUcC", "áéíóúÁÉÍÓÚçÇ")
    alvo = math.ceil(len(t) * 1.4)
    return "[" + t.translate(troca) + "·" * max(1, alvo - len(t)) + "]"


PS = {k: pseudo(v) for k, v in PT.items()}
for k in ("SQL",):
    PS[k] = pseudo("SQL")
PS["9 barras · 57 de 60 linhas"] = "[9 bárrás · 57 dé 60 línhás·········]"
PS["Imagem copiada · 1.280 × 720"] = "[Ímágém cópíádá · 1.280 × 720··········]"


# --- os dados -------------------------------------------------------------------------------------------------------------
CIDADES = [("São Paulo", 141220.35), ("Salvador", 88410.12), ("Recife", 79906.44), ("Porto Alegre", 72118.90),
           ("Rio de Janeiro", 63540.00), ("Curitiba", 51877.25), ("Belo Horizonte", 34990.61), ("Fortaleza", 18300.00),
           ("Manaus", 8943.13)]
MESES = ["nov", "dez", "jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out"]
RECEITA = [41820, 58310, 38240, 40115, 46980, 44310, 49870, 47220, 52640, 55180, 53920, 60470]
CUSTO = [27410, 35880, 25120, 26340, 29870, 28400, 31250, 30110, 32980, 34120, 33460, 36210]
HIST = [18, 46, 92, 141, 168, 160, 139, 115, 92, 71, 54, 41, 31, 24, 17, 12, 8, 5, 4, 2]  # 20 faixas de 100, total 1.240


def lcg(semente):
    x = semente
    while True:
        x = (1103515245 * x + 12345) % 2147483648
        yield x / 2147483648


def pontos_vendas(n=420):
    r = lcg(7)
    out = []
    for _ in range(n):
        q = 1 + int(next(r) ** 1.6 * 39)
        preco = 8 + next(r) ** 2 * 60
        v = q * preco * (0.85 + next(r) * 0.3)
        out.append((q + (next(r) - .5) * .5, min(v, 2350)))
    return out


def pontos_telemetria(n=2600):
    r = lcg(11)
    out = []
    for _ in range(n):
        u = next(r)
        g = sum(next(r) for _ in range(4)) / 4 - .5
        t = 14 + u * 22
        um = 88 - (t - 14) * 1.9 + g * 26
        out.append((t, max(18, min(98, um))))
    return out


# --- os gráficos em SVG ---------------------------------------------------------------------------------------------------
def largura_texto(t, px=11.5):
    return sum(.62 if c.isupper() else .55 for c in t) * px


def passo_bom(maximo, n=5):
    bruto = maximo / n
    e = 10 ** math.floor(math.log10(bruto))
    for m in (1, 2, 2.5, 5, 10):
        if m * e >= bruto:
            return m * e
    return 10 * e


def rotulo_eixo(v):
    if v >= 10000 and v % 1000 == 0:
        return p.numero(v / 1000) + " mil"
    return p.numero(v)


def svg(w, h, corpo, aria):
    return f'<svg class="grafico" viewBox="0 0 {w} {h}" width="{w}" height="{h}" role="img" aria-label="{aria}">{corpo}</svg>'


def cabecalho(titulo, sub, x=16):
    if not titulo:
        return "", 0
    s = f'<text class="tit" x="{x}" y="24">{titulo}</text>'
    if sub:
        s += f'<text class="sub" x="{x}" y="41">{sub}</text>'
    return s, 56 if sub else 40


def barras(w, h, dados, titulo="", sub="", mini=False, foco=None, eixo="Soma de limite"):
    topo_txt, topo = cabecalho(titulo, sub)
    rot_w = max(largura_texto(n, 11.5) for n, _ in dados) + 14
    val_w = 0 if mini else max(largura_texto(p.numero(v, 2), 11) for _, v in dados) + 12
    x0, x1 = 14 + rot_w, w - 16 - val_w
    y0, base = topo + (8 if mini else 6), h - (20 if mini else 34)
    passo = passo_bom(max(v for _, v in dados), 3 if mini else 5)
    vmax = passo * math.ceil(max(v for _, v in dados) / passo)
    linha = (base - y0) / len(dados)
    alt = min(22, linha * .62)
    out = [topo_txt]
    k = 0
    while k * passo <= vmax:
        x = x0 + (x1 - x0) * k * passo / vmax
        out.append(f'<line class="grade-l" x1="{x:.1f}" y1="{y0}" x2="{x:.1f}" y2="{base}"/>')
        if not mini or k % 2 == 0:
            out.append(f'<text x="{x:.1f}" y="{base + 15}" text-anchor="middle">{rotulo_eixo(k * passo)}</text>')
        k += 1
    for i, (nome, v) in enumerate(dados):
        cy = y0 + linha * (i + .5)
        bw = (x1 - x0) * v / vmax
        op = 1 if foco in (None, i) else .55
        out.append(f'<text class="rot" x="{x0 - 8:.1f}" y="{cy + 4:.1f}" text-anchor="end">{nome}</text>')
        out.append(f'<path d="M{x0:.1f} {cy - alt / 2:.1f}h{bw - 3:.1f}a3 3 0 0 1 3 3v{alt - 6:.1f}a3 3 0 0 1-3 3h-{bw - 3:.1f}z" '
                   f'style="fill:var(--s1);opacity:{op}"/>')
        if not mini:
            out.append(f'<text class="val" x="{x0 + bw + 6:.1f}" y="{cy + 4:.1f}">{p.numero(v, 2)}</text>')
    out.append(f'<line class="base" x1="{x0}" y1="{y0 - 4}" x2="{x0}" y2="{base}"/>')
    if not mini:
        out.append(f'<text class="eixo" x="{(x0 + x1) / 2:.1f}" y="{h - 6}" text-anchor="middle">{eixo}</text>')
    return svg(w, h, "".join(out), titulo or "Prévia do gráfico")


def eixos_xy(out, w, h, x0, x1, y0, base, ymax, passo_y, xticks, mini, ytit="", xtit=""):
    k = 0
    while k * passo_y <= ymax + 1e-9:
        y = base - (base - y0) * k * passo_y / ymax
        out.append(f'<line class="grade-l" x1="{x0}" y1="{y:.1f}" x2="{x1}" y2="{y:.1f}"/>')
        out.append(f'<text x="{x0 - 8}" y="{y + 4:.1f}" text-anchor="end">{rotulo_eixo(k * passo_y)}</text>')
        k += 1
    out.append(f'<line class="base" x1="{x0}" y1="{base}" x2="{x1}" y2="{base}"/>')
    for x, t in xticks:
        out.append(f'<text x="{x:.1f}" y="{base + 15}" text-anchor="middle">{t}</text>')
    if not mini and xtit:
        out.append(f'<text class="eixo" x="{(x0 + x1) / 2:.1f}" y="{h - 6}" text-anchor="middle">{xtit}</text>')
    if not mini and ytit:
        out.append(f'<text class="eixo" transform="translate(14 {(y0 + base) / 2:.1f}) rotate(-90)" text-anchor="middle">{ytit}</text>')


def linhas(w, h, titulo="", sub="", mini=False, cursor=None):
    topo_txt, topo = cabecalho(titulo, sub)
    x0, x1 = (52 if mini else 74), w - 18
    y0, base = topo + 10, h - (22 if mini else 36)
    ymax, passo = 70000, (35000 if mini else 10000)
    out = [topo_txt]
    xs = [x0 + (x1 - x0) * i / (len(MESES) - 1) for i in range(len(MESES))]
    ticks = [(xs[i], MESES[i] + ("/26" if i == 2 else "/25" if i == 0 else "")) for i in range(len(MESES)) if not mini or i % 3 == 0]
    eixos_xy(out, w, h, x0, x1, y0, base, ymax, passo, ticks, mini, "R$", "mes")
    for serie, cor in ((RECEITA, "--s1"), (CUSTO, "--s2")):
        pts = " ".join(f"{xs[i]:.1f},{base - (base - y0) * v / ymax:.1f}" for i, v in enumerate(serie))
        out.append(f'<polyline points="{pts}" style="fill:none;stroke:var({cor});stroke-width:2;stroke-linejoin:round;stroke-linecap:round"/>')
        if not mini:
            for i, v in enumerate(serie):
                out.append(f'<circle cx="{xs[i]:.1f}" cy="{base - (base - y0) * v / ymax:.1f}" r="2.6" style="fill:var(--c);stroke:var({cor});stroke-width:1.6"/>')
    if cursor is not None:
        x = xs[cursor]
        out.append(f'<line class="guia" x1="{x:.1f}" y1="{y0}" x2="{x:.1f}" y2="{base}"/>')
        for serie, cor in ((RECEITA, "--s1"), (CUSTO, "--s2")):
            out.append(f'<circle cx="{x:.1f}" cy="{base - (base - y0) * serie[cursor] / ymax:.1f}" r="4.5" style="fill:var({cor});stroke:var(--c);stroke-width:2"/>')
    return svg(w, h, "".join(out), titulo or "Prévia do gráfico")


def histograma(w, h, titulo="", sub="", mini=False, foco=None):
    topo_txt, topo = cabecalho(titulo, sub)
    x0, x1 = (40 if mini else 58), w - 18
    y0, base = topo + 10, h - (22 if mini else 36)
    ymax, passo = 180, (90 if mini else 30)
    out = [topo_txt]
    bw = (x1 - x0) / len(HIST)
    ticks = [(x0 + bw * i, rotulo_eixo(i * 100)) for i in range(0, len(HIST) + 1, 10 if mini else 4)]
    eixos_xy(out, w, h, x0, x1, y0, base, ymax, passo, ticks, mini, "pedidos", "total")
    for i, c in enumerate(HIST):
        a = (base - y0) * c / ymax
        op = 1 if foco in (None, i) else .6
        out.append(f'<rect x="{x0 + bw * i + .5:.1f}" y="{base - a:.1f}" width="{bw - 1:.1f}" height="{a:.1f}" rx="1.5" style="fill:var(--s1);opacity:{op}"/>')
    return svg(w, h, "".join(out), titulo or "Prévia do gráfico")


def dispersao(w, h, pts, xmax, ymax, passo_x, passo_y, titulo="", sub="", mini=False, xtit="", ytit="", alfa=.55, r=2.4, nota=""):
    topo_txt, topo = cabecalho(titulo, sub)
    x0, x1 = (44 if mini else 64), w - 18
    y0, base = topo + 10, h - (22 if mini else 36)
    out = [topo_txt]
    ticks, k = [], 0
    while k * passo_x <= xmax + 1e-9:
        ticks.append((x0 + (x1 - x0) * k * passo_x / xmax, rotulo_eixo(k * passo_x)))
        k += 1
    eixos_xy(out, w, h, x0, x1, y0, base, ymax, passo_y, ticks, mini, ytit, xtit)
    for x, y in pts:
        out.append(f'<circle cx="{x0 + (x1 - x0) * x / xmax:.1f}" cy="{base - (base - y0) * y / ymax:.1f}" r="{r}" style="fill:var(--s1);opacity:{alfa}"/>')
    if nota:
        out.append(f'<text class="sub" x="{x1}" y="{topo - 15 if topo else 14}" text-anchor="end">{nota}</text>')
    return svg(w, h, "".join(out), titulo or "Prévia do gráfico")


# --- as Propriedades com a vista Gráfico -----------------------------------------------------------------------------------
def segmentado_vistas(T, ativa="Gráfico", icones=False):
    nomes = ["Resumo", "Colunas", "Perfil", "Índices", "SQL", "Gráfico"]
    if icones:
        # os de hoje (Vista::icone no editor) e, para as duas vistas novas, a Caixa e as Colunas da biblioteca
        icos = {"Resumo": p.sk(p.icone_app("tabela")), "Colunas": p.sk(p.icone_app("estrutura")), "Perfil": ic("g_caixa"),
                "Índices": p.sk(p.icone_app("indices")), "SQL": p.folha("SQL"), "Gráfico": ic("g_colunas")}
        bts = "".join(f'<button{" class=\"on\"" if n == ativa else ""} role="tab" aria-label="{T[n]}" title="{T[n]}">{icos[n]}</button>'
                      for n in nomes)
        return f'<div class="segv"><div class="seg seg-ic" role="tablist" aria-label="O que a tabela mostra">{bts}</div></div>'
    bts = "".join(f'<button{" class=\"on\"" if n == ativa else ""} role="tab" aria-selected="{"true" if n == ativa else "false"}">{T[n]}</button>'
                  for n in nomes)
    return (f'<div class="segv"><div class="seg" style="display:grid;grid-auto-flow:column;grid-auto-columns:1fr;width:100%" role="tablist" '
            f'aria-label="O que a tabela mostra">{bts}</div></div>')


def seg_tipos(T, ativo):
    bts = "".join(f'<button{" class=\"on\"" if k == ativo else ""} role="tab" aria-selected="{"true" if k == ativo else "false"}" '
                  f'aria-label="{T[n]}" title="{T[n]}">{ic(f, 20)}</button>' for k, n, f in TIPOS)
    return f'<div class="seg seg-ic" role="tablist" aria-label="{T["Tipo"]}">{bts}</div>'


def lc(rotulo, controle, valor=""):
    v = f'<span class="v">{valor}</span>' if valor else ""
    return f'<div class="lc"><div class="r"><span>{rotulo}</span>{v}</div>{controle}</div>'


def campos_barras(T):
    return (lc(T["Categorias"], p.campo("cidade", T["Categorias"], p.P_TXT))
            + lc(T["Valores"], p.campo("limite", T["Valores"], p.P_DEC))
            + '<div class="duas">' + lc(T["Agregar"], p.campo(T["Soma"], T["Agregar"])) + lc(T["Ordem"], p.campo(T["Maior primeiro"], T["Ordem"])) + "</div>")


def acoes(T, aba=True):
    a = f'<button class="btn pri inteiro" title="O gráfico grande numa aba de documento">{T["Abrir em aba"]}</button>' if aba else ""
    return (f'<div class="acoes-g">{a}<button class="btn pq" title="Copia o gráfico como imagem (PNG), no tamanho da aba">{T["Copiar a imagem"]}</button>'
            f'<button class="btn pq" title="Salva o gráfico num arquivo PNG">{T["Salvar como PNG"]}</button>'
            f'<button class="btn sutil pq inteiro" title="Gera um script com pandas e Seaborn que lê estes dados e roda no Python da máquina; sem Python, copia o script">'
            f'{T["Abrir no Python"]}</button></div>')


def previa(T, grafico, pe=""):
    pe_ = f'<span class="pe">{pe}</span>' if pe else ""
    return lc(T["Prévia"], f'<div class="previa" title="{T["Abrir em aba"]} (duplo clique)">{grafico}{pe_}</div>')


def doca_prop(T, objeto, conteudo, largura=340, icones=True, estilo=""):
    destacar = p.botao("Destacar em janela", classe="ib pq")
    st = f";{estilo}" if estilo else ""
    return (f'<div class="doca doca-p" style="width:{largura}px{st}"><div role="tablist" aria-label="Painéis à direita" class="abas-doca">'
            f'<div class="aba on" role="presentation"><a href="#" role="tab" aria-selected="true">{T["Propriedades"]}<span class="sub">{objeto}</span></a></div>'
            f'<span style="flex:1"></span>{destacar}</div>'
            f'<aside class="vidro" aria-label="{T["Propriedades"]}" style="border-radius:10px">'
            f'{segmentado_vistas(T, icones=icones)}{conteudo}</aside></div>')


def vista(T, tipo, campos, prev="", aba=True):
    nome = dict((k, n) for k, n, _ in TIPOS)[tipo]
    return (f'<div class="vg">{lc(T["Tipo"], seg_tipos(T, tipo), T[nome])}{campos}{prev}{acoes(T, aba)}</div>')


# --- (a) a vista Gráfico nas Propriedades ------------------------------------------------------------------------------
NOMES = ["Lucas Brandão", "Carla Ibiapina", "Bruno Brandão", "Olívia Lacerda", "Bruno Ibiapina", "Rafael Ibiapina", "Gabriela Guimarães",
         "João Esteves", "Marina Lacerda", "Pedro Guimarães", "Ana Coutinho", "Helena Prado", "Caio Barreto", "Patrícia Lins"]
CID = ["Porto Alegre", "Recife", "Recife", "Curitiba", "Porto Alegre", "São Paulo", "Rio de Janeiro", "Belo Horizonte", "São Paulo",
       "Salvador", "São Paulo", "Salvador", "Manaus", "Fortaleza"]
LIM = [154.03, 14542.56, 13741.03, 17051, 4321.49, 10803.61, None, 2024.53, 11880.4, 9204.77, 15320, 7710.25, 4471.5, 9150]


def grade_clientes(n=14):
    th = (f'<thead><tr><th class="num" style="width:56px">{p.P_INT}id</th><th style="width:200px">{p.P_TXT}nome</th>'
          f'<th style="width:160px">{p.P_TXT}cidade</th><th class="num" style="width:130px">{p.P_DEC}limite</th><th></th></tr></thead>')
    linhas = []
    for i in range(n):
        lim = '<td class="num nulo">NULL</td>' if LIM[i] is None else f'<td class="num">{p.numero(LIM[i], 2)}</td>'
        cid_cls = ' class="selcol"'
        linhas.append(f'<tr><td class="num">{i + 1}</td><td>{NOMES[i]}</td><td{cid_cls}>{CID[i]}</td>{lim}<td></td></tr>')
    return '<table class="grade">' + th + "<tbody>" + "".join(linhas) + "</tbody></table>"


def barra_dados(exportar="Exportar clientes"):
    return p.barra(p.botao("Voltar"), p.botao("Avançar", desligado=True), S, p.botao("Abrir pasta"), p.botao("Abrir arquivo"),
                   p.botao("Salvar", desligado=True), S, p.botao("Nova linha"), p.botao("Nova coluna", desligado=True),
                   p.botao("Duplicar"), p.botao("Excluir"), p.botao("Desfazer", desligado=True), p.botao("Refazer", desligado=True), S,
                   p.botao("Filtrar"), p.botao("Atualizar"), S, p.botao("Exportar", dica=exportar))


def cena_a():
    T = PT
    prev = previa(T, barras(314, 184, CIDADES, mini=True), "")
    conteudo = vista(T, "barras", campos_barras(T), prev + f'<span style="color:var(--t3);font-size:11.5px;margin-top:-4px">{T["9 barras · 57 de 60 linhas"]}</span>')
    abas = p.abas(p.aba(p.sk(p.BANCO16), "exemplo.db", dica="C:\\dados\\exemplo.db"),
                  p.aba(p.sk(p.TABELA16), "clientes", on=True, dica="C:\\dados\\exemplo.db › clientes"))
    doc = ('<div class="doc" style="flex:1">' + '<div style="flex:1;min-height:0;overflow:hidden;background:var(--c)">' + grade_clientes() + "</div>"
           '<div class="rodape"><span>Linhas 1–14 de 60</span><span class="dir">cidade selecionada</span></div></div>')
    janela = (barra_dados() + '<div style="display:flex;gap:8px;height:700px;flex:none"><section style="flex:1;min-width:0;display:flex;flex-direction:column">'
              + abas + doc + "</section>" + doca_prop(T, "clientes", conteudo) + "</div>")
    return p.estado(
        "A vista Gráfico nas Propriedades",
        "A sexta vista das Propriedades (Alt+6), depois do Perfil (Alt+5), para a tabela, a visão, o CSV, a planilha e o resultado "
        "da Consulta. Seis vistas não cabem com nome na doca: o segmentado delas passa a só ícones, como a doca estreita já faz "
        "(Resumo, Colunas, Índices e SQL com os sólidos de hoje; Perfil com a Caixa e Gráfico com as Colunas, da biblioteca), e por "
        "extenso na janela própria. Em cima, o Tipo: os quatro sólidos da biblioteca num segmentado só de ícones, com o nome na dica e o nome do "
        "escolhido à direita do rótulo (cabe em qualquer idioma). Embaixo, as colunas do tipo, com a pastilha do tipo da coluna: "
        "Categorias aceita texto, data e inteiro; Valores, só números. Ao abrir, o app já escolhe: a coluna selecionada na grade "
        "(aqui, cidade) e a primeira numérica. Agregar e Ordem dividem a linha e descem uma embaixo da outra quando o texto não cabe. "
        "A prévia (a largura da doca, 184 px de altura) não tem títulos de eixo nem valores nas barras; o duplo clique nela ou "
        "\"Abrir em aba\" (o botão principal) abre o grande. Copiar a imagem e Salvar como PNG dividem a linha; o Abrir no Python "
        "é sutil, embaixo, com a explicação na dica e não numa legenda. A contagem diz quantas linhas entraram: os 3 limites NULL "
        "ficam de fora. A doca alarga para 340 px com a vista Gráfico ou Perfil, como faz a janela própria.",
        janela, largo=True)


# --- (b) o gráfico em aba -------------------------------------------------------------------------------------------------
def cena_b():
    T = PT
    conteudo = vista(T, "barras", campos_barras(T), aba=False)
    gw, gh = 1356 - 340 - 8 - 4, 560
    g = barras(gw, gh, CIDADES, "Soma de limite por cidade", "clientes · 57 de 60 linhas (3 com limite NULL ficaram de fora)", foco=2)
    abas = p.abas(p.aba(p.sk(p.TABELA16), "clientes", dica="C:\\dados\\exemplo.db › clientes"),
                  p.aba(ic("g_barras"), "Gráfico de clientes", on=True, dica="Gráfico de clientes · C:\\dados\\exemplo.db › clientes"))
    dica = ('<div role="tooltip" class="flut dica-g" style="left:672px;top:150px"><strong>Recife</strong>'
            '<span class="l"><i class="cor" style="background:var(--s1)"></i>Soma de limite<b>79.906,44</b></span>'
            '<span class="l" style="padding-left:16px">8 clientes</span></div>')
    doc = ('<div class="doc" style="flex:1"><div class="doc-g">' + g + dica + "</div>"
           '<div class="rodape"><span>9 barras · 57 de 60 linhas · 4 ms</span><span class="dir">Recife · 79.906,44</span></div></div>')
    barra = p.barra(p.botao("Voltar"), p.botao("Avançar", desligado=True), S, p.botao("Abrir pasta"), p.botao("Abrir arquivo"),
                    p.botao("Salvar", desligado=True), S, p.botao("Novo", desligado=True), p.botao("Nova coluna", desligado=True),
                    p.botao("Duplicar", desligado=True), p.botao("Excluir", desligado=True), p.botao("Desfazer", desligado=True),
                    p.botao("Refazer", desligado=True), S, p.botao("Filtrar", desligado=True),
                    p.botao("Atualizar", dica="Ler clientes de novo (F5)"), S,
                    p.botao("Exportar", dica="Salvar o gráfico como PNG"))
    janela = (barra + '<div style="display:flex;gap:8px;height:660px;flex:none"><section style="flex:1;min-width:0;display:flex;flex-direction:column">'
              + abas + doc + "</section>" + doca_prop(T, "Gráfico de clientes", conteudo) + "</div>")
    return p.estado(
        "O gráfico em aba",
        "Uma aba de documento como as outras, com endereço próprio: Voltar e Avançar passam por ela, e fechar não pergunta nada (o "
        "gráfico não se salva; o que fica é a escolha das colunas, guardada por objeto). O título da aba é \"Gráfico de clientes\", com o "
        "sólido do tipo escolhido; a dica tem o caminho inteiro. Sem faixa de cima: o tipo e as colunas ficam nas Propriedades, na "
        "mesma vista Gráfico, agora sem a prévia e sem o \"Abrir em aba\". O título e a fonte vão dentro do gráfico, porque a imagem "
        "copiada e o PNG precisam deles. Na barra, Atualizar lê a fonte de novo e Exportar salva o PNG; o resto desliga. Arrastar "
        "move, a roda amplia, o duplo clique restaura (as três na dica do gráfico). A dica do ponto é a das camadas (vidro), com a "
        "categoria, a série com a cor e a contagem; o rodapé conta à esquerda e mostra o ponto sob o cursor à direita (U4). Os "
        "rótulos das categorias medem o texto, sem coluna de largura fixa.",
        janela, largo=True)


# --- (c a f) os quatro tipos ----------------------------------------------------------------------------------------------
GW, GH = 624, 330


def cartao(campos, grafico, rodape_esq, rodape_dir="", legenda=""):
    leg = f'<div class="legenda-s">{legenda}</div>' if legenda else ""
    rd = f'<span class="dir">{rodape_dir}</span>' if rodape_dir else ""
    return (f'<div class="cartao-g"><div class="campos">{campos}</div>{leg}<div class="doc"><div class="doc-g">{grafico}</div>'
            f'<div class="rodape"><span>{rodape_esq}</span>{rd}</div></div></div>')


def lcc(rotulo, controle):
    return f'<div class="lc"><span>{rotulo}</span>{controle}</div>'


def cena_barras():
    campos = (lcc("Categorias", p.campo("cidade", "Categorias", p.P_TXT)) + lcc("Valores", p.campo("limite", "Valores", p.P_DEC))
              + lcc("Agregar", p.campo("Soma", "Agregar")) + lcc("Ordem", p.campo("Maior primeiro", "Ordem")))
    g = barras(GW, GH, CIDADES, "Soma de limite por cidade", "clientes · 57 de 60 linhas")
    return p.estado(
        "Barras",
        "Uma barra por categoria, deitada: o nome da categoria cabe inteiro à esquerda, em qualquer idioma, e o valor fica na ponta. "
        "Agregar: Soma, Média, Contagem, Mínimo e Máximo (Contagem dispensa os Valores). Ordem: Maior primeiro, Menor primeiro, A a Z e "
        "a da fonte. Até 30 categorias; as outras somam numa barra \"Outras (12)\" em --t3, para nada sumir sem aviso. A agregação "
        "roda no banco (ou na bancada, para o CSV e a planilha), não na tela.",
        cartao(campos, g, "9 barras · 57 de 60 linhas"))


def cena_linhas():
    p_mes = p.P_DATA
    chips = ('<div class="chips">'
             f'<span class="chip-c"><i class="cor" style="background:var(--s1)"></i>{p.P_DEC}receita<button class="x" aria-label="Tirar receita">{p.FECHAR_X}</button></span>'
             f'<span class="chip-c"><i class="cor" style="background:var(--s2)"></i>{p.P_DEC}custo<button class="x" aria-label="Tirar custo">{p.FECHAR_X}</button></span>'
             '<button class="btn sutil pq">+ Valores</button></div>')
    campos = lcc("Eixo X", p.campo("mes", "Eixo X", p_mes)) + f'<div class="lc" style="flex:2 1 300px"><span>Valores</span>{chips}</div>'
    g = linhas(GW, GH, "Receita e custo por mês", "Consulta 2 · comando 1 · 12 linhas", cursor=11)
    dica = ('<div role="tooltip" class="flut dica-g" style="right:46px;top:150px"><strong>out/26</strong>'
            '<span class="l"><i class="cor" style="background:var(--s1)"></i>receita<b>60.470,00</b></span>'
            '<span class="l"><i class="cor" style="background:var(--s2)"></i>custo<b>36.210,00</b></span></div>')
    leg = '<span><i style="background:var(--s1)"></i>receita</span><span><i style="background:var(--s2)"></i>custo</span>'
    cart = cartao(campos, g, "2 séries · 12 pontos", "out/26", leg).replace("</svg></div>", "</svg>" + dica + "</div>", 1)
    return p.estado(
        "Linhas, do resultado de uma consulta",
        "O eixo X aceita data, número ou texto ordenado; os Valores são até três colunas de números, uma cor cada (--s1 é o destaque; "
        "--s2 e --s3, verde-azulado e âmbar, valem nos dois temas). Com duas ou mais séries entra a legenda, em cima. A fonte aqui é "
        "o resultado do comando 1 da Consulta 2: a vista Gráfico das Propriedades da Consulta segue o comando escolhido nos "
        "resultados. A dica junta as séries do mesmo X, e a guia tracejada marca onde o cursor está. X repetido agrega (Soma, como "
        "nas barras).",
        cart)


def cena_histograma():
    campos = lcc("Valores", p.campo("total", "Valores", p.P_DEC)) + lcc("Faixas", p.campo("Automático (20)", "Faixas"))
    g = histograma(GW, GH, "Distribuição de total", "pedidos · 1.240 linhas · faixas de 100", foco=4)
    dica = ('<div role="tooltip" class="flut dica-g" style="left:206px;top:96px"><strong>400 a 500</strong>'
            '<span class="l"><i class="cor" style="background:var(--s1)"></i>pedidos<b>168</b></span></div>')
    cart = cartao(campos, g, "20 faixas · 1.240 linhas", "400 a 500 · 168").replace("</svg></div>", "</svg>" + dica + "</div>", 1)
    return p.estado(
        "Histograma",
        "Uma coluna de números em faixas de largura igual. Faixas: Automático (a regra de Freedman–Diaconis, arredondada para um "
        "passo redondo, entre 5 e 50), 10, 20 ou 50. As barras encostam (1 px de vão), porque a faixa é contínua. É o mesmo desenho "
        "do Perfil da coluna, em grande; os nulos ficam de fora e entram na contagem do rodapé.",
        cart)


def cena_dispersao():
    campos = lcc("Eixo X", p.campo("quantidade", "Eixo X", p.P_INT)) + lcc("Eixo Y", p.campo("valor", "Eixo Y", p.P_DEC))
    g = dispersao(GW, GH, pontos_vendas(), 40, 2500, 5, 500, "valor por quantidade", "vendas_2025.csv · 1.180 pontos",
                  xtit="quantidade", ytit="valor")
    return p.estado(
        "Dispersão",
        "Dois números, um ponto por linha, a 55% de opacidade para os pontos que se sobrepõem aparecerem mais escuros. Até 100 mil "
        "pontos, todos; acima disso, uma amostra (cena i). A fonte aqui é um CSV: o gráfico lê o mesmo arquivo que a grade, pela "
        "FonteDeLinhas, e o arquivo remoto do SSH (pelo cache) funciona igual.",
        cartao(campos, g, "1.180 pontos"))


# --- (g, h, i) vazio, carregando e amostra --------------------------------------------------------------------------------
def cena_vazio():
    T = PT
    sem_num = ('<div class="vg"><div class="vazio-g">' + p.sk(p.forma("st_info", 24), 24)
               + '<span>tags_produtos não tem coluna de números.</span>'
               '<span class="t3">Barras com Contagem funcionam com qualquer coluna.</span>'
               '<button class="btn pq">Contar por categoria</button></div></div>')
    sem_esc = ('<div class="vg">' + lc(T["Tipo"], seg_tipos(T, "dispersao"), T["Dispersão"])
               + lc("Eixo X", p.campo("quantidade", "Eixo X", p.P_INT))
               + lc("Eixo Y", p.campo('<span style="color:var(--t3)">Escolha uma coluna</span>', "Eixo Y"))
               + lc(T["Prévia"], '<div class="vazio-g" style="height:120px"><span class="t3">Falta a coluna do eixo Y.</span></div>')
               + acoes(T).replace('<button class="btn', '<button disabled="" class="btn') + "</div>")
    palco = (f'<div class="par">'
             f'<div class="etapa"><span class="tit">1 · Sem coluna de números</span>{doca_prop(T, "tags_produtos", sem_num, 300, estilo="height:300px")}</div>'
             f'<div class="etapa"><span class="tit">2 · Falta uma coluna</span>{doca_prop(T, "vendas_2025.csv", sem_esc, 300)}</div></div>')
    return p.estado(
        "Vazio",
        "Sem coluna de números, a vista diz isso na caixa de nota e oferece o único gráfico possível, as barras com Contagem. Com "
        "uma escolha faltando, o campo diz o que escolher, a prévia diz o que falta e as ações desligam (a dica diz o motivo). Uma "
        "fonte sem linhas mostra os eixos vazios com \"Nenhuma linha em …\", como a U5.",
        palco)


def cena_carregando():
    T = PT
    esq = barras(300, 150, CIDADES, mini=True).replace('style="fill:var(--s1);opacity:1"', 'style="fill:var(--t3);opacity:1"')
    prev = (f'<div class="previa" style="height:150px"><div class="esqueleto">{esq}</div>'
            f'<div class="carrega">{p.GIRA}</div></div>')
    doc = ('<div class="doc"><div class="doc-g" style="height:250px;flex:none"><div class="carrega">' + p.GIRA
           + '<span>Agregando leituras… 1,2 de 2,3 milhões de linhas</span>'
           '<div class="barra" role="progressbar" aria-valuenow="52" aria-valuemin="0" aria-valuemax="100"><div style="width:52%"></div></div>'
           '<button class="btn risco pq">Interromper</button></div></div>'
           '<div class="rodape"><span>Agregando · 1,8 s</span></div></div>')
    palco = (f'<div class="etapa"><span class="tit">1 · A prévia nas Propriedades</span><div style="width:300px">{lc(T["Prévia"], prev)}</div></div>'
             f'<div class="etapa" style="margin-top:6px"><span class="tit">2 · A aba, com uma fonte grande</span>{doc}</div>')
    return p.estado(
        "Carregando",
        "A prévia fica com o desenho anterior a 35% e o arco no meio; não tem progresso nem Interromper (é pequena e se refaz a cada "
        "troca). A aba espera 400 ms antes de mostrar alguma coisa; depois, o arco, o andamento em linhas, a barra e o Interromper "
        "(botão de risco, Esc), como a Consulta executando (N3). O rodapé conta o tempo. Interromper deixa a aba com o último gráfico "
        "pronto, ou vazia com \"Interrompido\" se não havia nenhum.",
        palco)


def cena_amostra():
    T = PT
    gw, gh = 1356 - 340 - 8 - 2, 520
    g = dispersao(gw, gh, pontos_telemetria(), 40, 100, 5, 20, "umidade por temperatura", "telemetria.db › leituras · amostra de 100.000 de 2.347.118 pontos",
                  xtit="temperatura", ytit="umidade", alfa=.22, r=1.7)
    campos = lc("Eixo X", p.campo("temperatura", "Eixo X", p.P_DEC)) + lc("Eixo Y", p.campo("umidade", "Eixo Y", p.P_DEC))
    conteudo = vista(T, "dispersao", campos, aba=False)
    amostra = (f'<span class="amostra" title="100.000 pontos escolhidos ao acaso entre os 2.347.118, os mesmos a cada abertura. '
               f'O PNG leva a amostra; o Abrir no Python lê todos.">{p.sk(p.forma("st_info", 16))}Amostra de 100.000 de 2.347.118 pontos</span>')
    doc = ('<div class="doc" style="flex:1"><div class="doc-g">' + g + "</div>"
           f'<div class="rodape">{amostra}<span class="dir">31,4 · 52,0</span></div></div>')
    abas = p.abas(p.aba(p.sk(p.TABELA16), "leituras", dica="C:\\dados\\telemetria.db › leituras"),
                  p.aba(ic("g_dispersao"), "Gráfico de leituras", on=True, dica="Gráfico de leituras · C:\\dados\\telemetria.db › leituras"))
    janela = ('<div style="display:flex;gap:8px;height:600px;flex:none"><section style="flex:1;min-width:0;display:flex;flex-direction:column">'
              + abas + doc + "</section>" + doca_prop(T, "Gráfico de leituras", conteudo) + "</div>")
    return p.estado(
        "Amostra: a dispersão acima de 100 mil pontos",
        "Acima de 100 mil linhas, a dispersão desenha uma amostra de 100.000 pontos, sorteada no banco ou na bancada com semente fixa "
        "(a mesma a cada abertura, para o gráfico não mudar sozinho). A etiqueta âmbar no rodapé, à esquerda, diz isso com os dois "
        "números; a dica explica o resto. O subtítulo dentro do gráfico também diz \"amostra\", para a imagem copiada e o PNG não "
        "mentirem. A opacidade dos pontos cai para 22% quando há muitos. As barras, as linhas e o histograma não amostram: agregam "
        "todas as linhas.",
        janela, largo=True)


# --- (j) copiar, salvar e Abrir no Python ------------------------------------------------------------------------------------
def cena_acoes():
    T = PT
    menu = ('<div role="menu" aria-label="Gráfico" class="flut menu-ctx menu-g" style="position:relative">'
            + p.mi(T["Copiar a imagem"], "Ctrl+C") + p.mi(T["Salvar como PNG"], "Ctrl+Shift+S", icone=p.sil("Exportar"))
            + p.mi(T["Abrir no Python"]) + p.SEP + p.mi(T["Restaurar o zoom"], "duplo clique") + "</div>")
    toasts = ('<div class="pilha">'
              + p.toast(T["Imagem copiada · 1.280 × 720"])
              + p.toast("Salvo em C:\\dados\\clientes_limite_por_cidade.png", acao="Mostrar na pasta")
              + p.toast("O gráfico abriu no Python 3.12", icone=p.forma("st_info", 16))
              + p.toast("Python não encontrado. O script foi copiado.", icone=p.ATENCAO, acao="Ver o script")
              + "</div>")
    palco = (f'<div style="display:flex;gap:40px;align-items:flex-start"><div class="etapa"><span class="tit">1 · O menu do gráfico</span>{menu}</div>'
             f'<div class="etapa"><span class="tit">2 · Os avisos</span>{toasts}</div></div>')
    return p.estado(
        "Copiar a imagem, Salvar como PNG e Abrir no Python",
        "O clique direito no gráfico (na aba e na prévia) abre o menu; só o Salvar como PNG leva silhueta, porque é o Exportar da "
        "barra. Copiar a imagem leva um PNG no tamanho do gráfico na tela, a 2×, com o fundo do tema da tela. Salvar como PNG abre o "
        "diálogo do sistema com o nome sugerido (tabela_valores_por_categorias.png). Abrir no Python grava o script e os dados (o "
        "resultado da consulta, num CSV temporário) e roda no Python da máquina; sem Python, copia o script e o aviso oferece vê-lo. "
        "Os avisos são os flutuantes de sempre, com o ícone de estado.",
        palco, largo=True)


# --- (k) o texto 40% mais longo -------------------------------------------------------------------------------------------
def cena_pseudo():
    def lado_(T, titulo, icones):
        prev = previa(T, barras(314, 160, CIDADES, mini=True))
        conteudo = vista(T, "barras", campos_barras(T), prev + f'<span style="color:var(--t3);font-size:11.5px;margin-top:-4px">{T["9 barras · 57 de 60 linhas"]}</span>')
        return f'<div class="etapa"><span class="tit">{titulo}</span>{doca_prop(T, "clientes", conteudo, icones=icones)}</div>'

    menu = ('<div role="menu" class="flut menu-ctx menu-g" style="position:relative">'
            + p.mi(PS["Copiar a imagem"], "Ctrl+C") + p.mi(PS["Salvar como PNG"], "Ctrl+Shift+S", icone=p.sil("Exportar"))
            + p.mi(PS["Abrir no Python"]) + p.SEP + p.mi(PS["Restaurar o zoom"], "[dúpló clíqué·····]") + "</div>")
    extra = (f'<div class="etapa"><span class="tit">3 · O menu e o aviso</span><div class="pilha">{menu}{p.toast(PS["Imagem copiada · 1.280 × 720"])}</div></div>')
    palco = '<div style="display:flex;gap:24px;align-items:flex-start;flex-wrap:wrap">' + lado_(PT, "1 · Português", True) + lado_(PS, "2 · Pseudoidioma, 40% mais longo", True) + extra + "</div>"
    return p.estado(
        "Texto 40% mais longo (P7)",
        "O mesmo painel no pseudoidioma da P7 (\"[Ábrír ém ábá·····]\"). O que vem dos dados (cidade, limite, os números) não muda. Sem "
        "nenhuma largura fixa pelo texto: as vistas e o Tipo já são só ícones, com o nome na dica; o nome do tipo escolhido "
        "quebra a linha se precisar; Agregar e Ordem ficam lado a lado enquanto cada um tem 120 px e descem um embaixo do outro "
        "depois disso; o campo corta o valor com \"…\" e a dica tem o inteiro; Copiar a imagem e Salvar como PNG dividem a linha "
        "enquanto cabem e passam a uma linha cada depois. Os menus e os avisos medem o texto mais longo.",
        palco, largo=True)


# --- a página -------------------------------------------------------------------------------------------------------------
cenas = [cena_a(), cena_b(), cena_barras(), cena_linhas(), cena_histograma(), cena_dispersao(), cena_vazio(), cena_carregando(),
         cena_amostra(), cena_acoes(), cena_pseudo()]
quadro = '<div class="quadro">' + "".join(cenas) + "</div>"
texto = p.pagina("Gráficos: a vista Gráfico e o gráfico em aba", MARCA, PROPRIO, ALTURA, quadro, ("icVolH", "icVolR", "icVolV"))
p.conferir_refs(texto, ARQUIVO)
if "--medir" in sys.argv:
    print(texto)
    sys.exit(0)
p.gravar(ARQUIVO, texto, MARCA, {"expand": "fill", "h": ALTURA, "page": "futuro", "title": TITULO, "w": 1440, "x": 1520, "y": 0},
         aceitar_sem_marca="--substituir" in sys.argv)
