"""Gera o logo do DataBox no estilo dos ícones sólidos e escreve a prancheta Logo.dc.html.

Caixa de papelão aberta em isométrico; um banco (cilindro metálico com faixas) e uma tabela
(folha com cabeçalho) saindo dela; luz por trás dos dois.
"""
import math
import sys
from pathlib import Path

C30 = math.cos(math.radians(30))
PROJETO = Path(__file__).resolve().parent.parent / "canvas"


def proj(x, y, z):
    return (C30 * (x - y), 0.5 * (x + y) - z)


def fmt(v):
    s = f"{v:.1f}"
    if s == "-0.0":
        s = "0.0"
    return s[:-2] if s.endswith(".0") else s


def poli(pontos):
    return "M" + " L".join(f"{fmt(x)} {fmt(y)}" for x, y in pontos) + "Z"


def linha(p1, p2):
    return f"M{fmt(p1[0])} {fmt(p1[1])} L{fmt(p2[0])} {fmt(p2[1])}"


CONTORNO = 'stroke="#000000" style="stroke-opacity:{o};stroke-width:{w};stroke-linejoin:round"'


def contorno(o=".3", w="2.4"):
    return CONTORNO.format(o=o, w=w)


def gerar(simples=False, bruto=False):
    W = D = 150.0
    H = 100.0
    pts = []

    def p(x, y, z):
        q = proj(x, y, z)
        pts.append(q)
        return q

    B, R, L, F = p(0, 0, H), p(W, 0, H), p(0, D, H), p(W, D, H)
    R0, L0, F0, B0 = p(W, 0, 0), p(0, D, 0), p(W, D, 0), p(0, 0, 0)

    fl = 58.0 if not simples else 40.0
    th = math.radians(38)
    dx, dz = -fl * math.sin(th), fl * math.cos(th)
    abaTE = [B, L, p(dx, D, H + dz), p(dx, 0, H + dz)]
    abaTD = [B, R, p(W, dx, H + dz), p(0, dx, H + dz)]
    ff = 50.0 if not simples else 42.0
    tf = math.radians(146)
    dy, dz2 = ff * math.sin(tf), ff * math.cos(tf)
    abaFE = [L, F, p(W, D + dy, H + dz2), p(0, D + dy, H + dz2)]
    abaFD = [F, R, p(W + dy, 0, H + dz2), p(W + dy, D, H + dz2)]

    def inter(p1, p2, p3, p4):
        (x1, y1), (x2, y2), (x3, y3), (x4, y4) = p1, p2, p3, p4
        den = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4)
        t = ((x1 - x3) * (y3 - y4) - (y1 - y3) * (x3 - x4)) / den
        return (x1 + t * (x2 - x1), y1 + t * (y2 - y1))

    I1 = inter(L, F, L0, B0)
    I2 = inter(R, F, R0, B0)
    paredeE = [B, L, I1, B0]
    paredeD = [B, R, I2, B0]
    abertura = [B, R, F, L]
    faceE = [L, F, F0, L0]
    faceD = [F, R, R0, F0]

    # banco "gordinho": largura e altura visível parecidas, como o ícone do banco
    cx, cy, r = (56.0, 95.0, 52.0) if not simples else (66.0, 90.0, 56.0)
    zt = 146.0 if not simples else 150.0
    zb = 10.0
    assert cx - r > 2 and cy + r < D - 2 and cy - r > 30, "o banco precisa caber na caixa e não tocar a tabela"
    a, b = r * math.sqrt(2) * C30, r * math.sqrt(2) * 0.5
    topo = p(cx, cy, zt)
    base = proj(cx, cy, zb)
    pts += [(topo[0] - a, topo[1] - b), (topo[0] + a, topo[1] - b)]
    alturas = (zt - 27, zt - 54)
    faixas = [proj(cx, cy, z) for z in alturas]

    tab = lapis = lupa = graf = None

    def cartao(nome, phi_g, beta_g, larg, alt, x0, y0, z0):
        """Folha em pé num plano vertical girado phi_g a partir da parede do fundo e inclinada beta_g
        dentro do próprio plano; confere que cabe na caixa e não encosta no banco."""
        phi_, beta_ = math.radians(phi_g), math.radians(beta_g)
        hx, hy = math.cos(phi_), -math.sin(phi_)
        nx, ny = math.sin(phi_), math.cos(phi_)

        def ponto3(u, s):
            a_ = u * larg * math.cos(beta_) + s * alt * math.sin(beta_)
            return (x0 + a_ * hx, y0 + a_ * hy, z0 - u * larg * math.sin(beta_) + s * alt * math.cos(beta_))

        def ponto(u, s):
            return proj(*ponto3(u, s))

        assert (cx - x0) * nx + (cy - y0) * ny > r + 2, f"{nome} encostaria no banco"
        assert min(z0, z0 - larg * math.sin(beta_)) > 2, f"{nome} passaria do fundo"
        for u in (0, 1):
            s = (H - (z0 - u * larg * math.sin(beta_))) / (alt * math.cos(beta_))
            X, Y, _ = ponto3(u, s)
            assert 3 <= X <= W - 3 and 3 <= Y <= D - 3, (f"{nome} sairia da caixa no aro", u, X, Y)
        for uv in ((0, 0), (1, 0), (0, 1), (1, 1)):
            pts.append(ponto(*uv))
        return ponto, proj(-nx * 5, -ny * 5, 0), (nx, ny, x0, y0)

    if not simples:
        # o conjunto sai da caixa como um buquê: lápis e gráfico no fundo inclinados para a esquerda,
        # banco à frente, planilha à direita e a lupa erguida à direita, na frente da planilha
        tab, espessura, plano_tab = cartao("a planilha", 22, 6, 72.0, 130.0, 58.0, 34.0, 22.0)
        graf, espessura_g, plano_g = cartao("o gráfico", 10, -5, 56.0, 150.0, 16.0, 16.0, 44.0)
        # o gráfico fica atrás da planilha, sem atravessá-la (inclui a espessura de 5 da planilha)
        nxp, nyp, xp, yp = plano_tab
        assert all((nxp * (X - xp) + nyp * (Y - yp)) < -6 for X, Y in ((16.0, 16.0), (16.0 + 56 * math.cos(math.radians(10)), 16.0 - 56 * math.sin(math.radians(10))))), "o gráfico atravessaria a planilha"

        # lápis no fundo, encostado no canto de trás, com a ponta para cima
        lapis = {"base": proj(24, 28, 34), "ang": math.radians(-22), "comp": 176.0, "larg": 13.0}
        dl = (math.sin(lapis["ang"]), -math.cos(lapis["ang"]))
        pts.append((lapis["base"][0] + dl[0] * lapis["comp"], lapis["base"][1] + dl[1] * lapis["comp"]))

        # lupa erguida à direita, inclinada para fora, com o cabo descendo para dentro da caixa
        lupa = {"centro": (86.0, -58.0), "raio": 33.0, "ang_cabo": math.radians(116), "rot": 18.0}
        lc = lupa["centro"]
        pts += [(lc[0] - lupa["raio"] * 1.15, lc[1] - lupa["raio"] * 1.15), (lc[0] + lupa["raio"] * 1.15, lc[1] + lupa["raio"] * 1.15)]

    if not simples:
        def caixa(nome, qs):
            print(f"  {nome}: x {min(q[0] for q in qs):.0f}..{max(q[0] for q in qs):.0f}  y {min(q[1] for q in qs):.0f}..{max(q[1] for q in qs):.0f}", file=sys.stderr)

        caixa("tampa do banco", [(topo[0] - a, topo[1] - b), (topo[0] + a, topo[1] + b)])
        caixa("planilha", [tab(u, s) for u in (0, 1) for s in (0, 1)])
        caixa("gráfico", [graf(u, s) for u in (0, 1) for s in (0, 1)])
        caixa("gráfico acima de s=.5", [graf(u, s) for u in (0, 1) for s in (.5, 1)])
        caixa("lupa", [(lc[0] - lupa["raio"], lc[1] - lupa["raio"]), (lc[0] + lupa["raio"], lc[1] + lupa["raio"])])
        caixa("lápis", [lapis["base"], (lapis["base"][0] + dl[0] * lapis["comp"], lapis["base"][1] + dl[1] * lapis["comp"])])
        caixa("aro (L, F, R)", [L, F, R])
    xs = [q[0] for q in pts]
    ys = [q[1] for q in pts]
    minx, maxx, miny, maxy = min(xs), max(xs), min(ys), max(ys)
    margem = 40 if not simples else 22
    escala = (512 - 2 * margem) / max(maxx - minx, maxy - miny)
    ox = 256 - (minx + maxx) / 2 * escala
    oy = 256 - (miny + maxy) / 2 * escala

    def T(q):
        return (q[0] * escala + ox, q[1] * escala + oy)

    def P(pontos):
        return poli([T(q) for q in pontos])

    out = []
    A = out.append

    A("@@camada:luz")
    # luz por trás (duas cores para comparar; a classe do svg escolhe)
    tx, ty = T(topo)
    cl = (256, 236)
    raio_luz = 222
    for cls, grad in (("luzA", "lgLuzA"), ("luzB", "lgLuzB")):
        A(f'<circle class="{cls}" cx="{fmt(cl[0])}" cy="{fmt(cl[1])}" r="{raio_luz}" fill="url(#{grad})"></circle>')

    A('<g filter="url(#lgSombra)">')  # @@grupo
    A("@@camada:abas de trás")
    # abas de trás (face de dentro)
    for aba, grad in ((abaTE, "lgAbaTE"), (abaTD, "lgAbaTD")):
        A(f'<path d="{P(aba)}" fill="url(#{grad})" {contorno()}></path>')
    bordas_tras = [linha(T(aba[2]), T(aba[3])) for aba in (abaTE, abaTD)]
    A(f'<path d="{" ".join(bordas_tras)}" stroke="#fff3dc" style="stroke-opacity:.75;stroke-width:3;stroke-linecap:round"></path>')
    A("@@camada:raios")
    # raios de luz saindo da caixa (a parte de dentro fica coberta pelo interior)
    if not simples:
        bxr, byr = T(proj(W * 0.5, D * 0.5, H * 0.35))
        for ang, abre in ((-44, 14), (-21, 10), (1, 15), (23, 10), (45, 13)):
            t1, t2 = math.radians(ang - abre / 2), math.radians(ang + abre / 2)
            comp = 250
            q1 = (bxr + comp * math.sin(t1), byr - comp * math.cos(t1))
            q2 = (bxr + comp * math.sin(t2), byr - comp * math.cos(t2))
            for cls, grad in (("luzA", "lgRaioA"), ("luzB", "lgRaioB")):
                A(f'<path class="{cls}" d="{poli([(bxr, byr), q1, q2])}" fill="url(#{grad})"></path>')
    A("@@camada:interior da caixa")
    # interior
    A(f'<path d="{P(abertura)}" fill="#3a2410"></path>')
    A(f'<path d="{P(paredeE)}" fill="url(#lgParedeE)"></path>')
    A(f'<path d="{P(paredeD)}" fill="url(#lgParedeD)"></path>')
    for cls, grad in (("luzA", "lgDentroA"), ("luzB", "lgDentroB")):
        A(f'<path class="{cls}" d="{P(abertura)}" fill="url(#{grad})"></path>')
    A(f'<path d="{P(abertura)}" fill="none" {contorno(".35")}></path>')

    A("@@camada:gráfico")
    # gráfico de barras no fundo (a planilha e o banco, desenhados depois, cobrem a parte de baixo)
    if graf:
        def pg(u, s):
            return T(graf(u, s))

        e = (espessura_g[0] * escala, espessura_g[1] * escala)
        cg = [pg(0, 0), pg(1, 0), pg(1, 1), pg(0, 1)]
        A(f'<path d="{poli([(x + e[0], y + e[1]) for x, y in cg])}" fill="#b9c1cd" {contorno()}></path>')
        A(f'<path d="{poli(cg)}" fill="url(#lgPapel)" {contorno()}></path>')
        A('<defs><linearGradient id="lgVolBarra" x1="0" y1="0" x2="1" y2="0"><stop offset="0" style="stop-color:#ffffff;stop-opacity:.4"></stop>'
          '<stop offset=".45" style="stop-color:#ffffff;stop-opacity:.04"></stop><stop offset="1" style="stop-color:#000000;stop-opacity:.2"></stop></linearGradient></defs>')
        base_s = 0.5
        barras = ((0.1, 0.27, 0.2, "#2c6fdc"), (0.32, 0.49, 0.34, "#1f8a8a"), (0.54, 0.71, 0.25, "#e6a133"), (0.76, 0.93, 0.42, "#2e9e5b"))
        for u0, u1, h_, cor in barras:
            bar = poli([pg(u0, base_s), pg(u1, base_s), pg(u1, base_s + h_), pg(u0, base_s + h_)])
            A(f'<path d="{bar}" fill="{cor}" {contorno(".3", "1.6")}></path>')
            A(f'<path d="{bar}" fill="url(#lgVolBarra)"></path>')
        A(f'<path d="{linha(pg(0.05, base_s), pg(0.97, base_s))} {linha(pg(0.05, base_s), pg(0.05, 0.96))}" stroke="#000000" style="stroke-opacity:.3;stroke-width:2.4;stroke-linecap:round"></path>')
        A(f'<path d="{linha(pg(0, 1), pg(1, 1))}" stroke="#ffffff" style="stroke-opacity:.9;stroke-width:3;stroke-linecap:round"></path>')

    A("@@camada:lápis")
    # lápis (no fundo: o banco e a planilha, desenhados depois, cobrem a parte de baixo)
    if lapis:
        bl = T(lapis["base"])
        ang = lapis["ang"]
        d = (math.sin(ang), -math.cos(ang))
        q = (math.cos(ang), math.sin(ang))
        L_, w_ = lapis["comp"] * escala, lapis["larg"] * escala

        def lp(t, k):
            return (bl[0] + d[0] * t + q[0] * k, bl[1] + d[1] * t + q[1] * k)

        fim_corpo, ini_grafite = L_ - 30 * escala, L_ - 9 * escala
        faces = ((-w_ / 2, -w_ / 6, "#ffd86b"), (-w_ / 6, w_ / 6, "#f2b829"), (w_ / 6, w_ / 2, "#cf8d12"))
        for k0, k1, cor in faces:
            A(f'<path d="{poli([lp(0, k0), lp(0, k1), lp(fim_corpo, k1), lp(fim_corpo, k0)])}" fill="{cor}"></path>')
        A(f'<path d="{poli([lp(fim_corpo, -w_ / 2), lp(fim_corpo, w_ / 2), lp(ini_grafite, w_ * .16), lp(ini_grafite, -w_ * .16)])}" fill="#f1cf9c"></path>')
        A(f'<path d="{poli([lp(fim_corpo, w_ / 6), lp(fim_corpo, w_ / 2), lp(ini_grafite, w_ * .16), lp(ini_grafite, w_ * .05)])}" fill="#000000" style="fill-opacity:.12"></path>')
        A(f'<path d="{poli([lp(ini_grafite, -w_ * .16), lp(ini_grafite, w_ * .16), lp(L_, 0)])}" fill="#3b3f46"></path>')
        A(f'<path d="{poli([lp(0, -w_ / 2), lp(0, w_ / 2), lp(fim_corpo, w_ / 2), lp(ini_grafite, w_ * .16), lp(L_, 0), lp(ini_grafite, -w_ * .16), lp(fim_corpo, -w_ / 2)])}" fill="none" {contorno()}></path>')
        A(f'<path d="{linha(lp(6, -w_ / 3), lp(fim_corpo - 6, -w_ / 3))}" stroke="#ffffff" style="stroke-opacity:.55;stroke-width:{fmt(w_ * .13)};stroke-linecap:round"></path>')

    A("@@camada:planilha")
    # tabela: folha com cabeçalho, como o ícone de tabela
    if tab:
        def pt(u, s):
            return T(tab(u, s))

        be, bd, te, td = pt(0, 0), pt(1, 0), pt(0, 1), pt(1, 1)
        e = (espessura[0] * escala, espessura[1] * escala)

        def desl(q):
            return (q[0] + e[0], q[1] + e[1])

        A(f'<path d="{poli([desl(be), desl(bd), desl(td), desl(te)])}" fill="#b9c1cd" {contorno()}></path>')
        A(f'<path d="{poli([be, bd, td, te])}" fill="url(#lgPapel)" {contorno()}></path>')
        A(f'<path d="{poli([pt(0, 0.78), pt(1, 0.78), pt(1, 1), pt(0, 1)])}" fill="#2c6fdc"></path>')
        A(f'<path d="{poli([pt(0, 0.89), pt(1, 0.89), pt(1, 1), pt(0, 1)])}" fill="url(#lgRealce)"></path>')
        A(f'<path d="{poli([be, bd, td, te])}" fill="none" {contorno()}></path>')
        grade = [linha(pt(0, s), pt(1, s)) for s in (0.585, 0.39, 0.195)]
        grade += [linha(pt(u, 0), pt(u, 0.78)) for u in (0.33, 0.66)]
        A(f'<path d="{" ".join(grade)}" stroke="#000000" style="stroke-opacity:.16;stroke-width:3"></path>')
        A(f'<path d="{linha(te, td)}" stroke="#ffffff" style="stroke-opacity:.9;stroke-width:3;stroke-linecap:round"></path>')

    A("@@camada:banco")
    # banco: cilindro metálico com faixas na cor de acento, como o ícone do banco
    _, by = T(base)
    A2, B2 = a * escala, b * escala
    corpo = (f"M{fmt(tx - A2)} {fmt(ty)} L{fmt(tx - A2)} {fmt(by)} A{fmt(A2)} {fmt(B2)} 0 0 0 {fmt(tx + A2)} {fmt(by)} "
             f"L{fmt(tx + A2)} {fmt(ty)} A{fmt(A2)} {fmt(B2)} 0 0 0 {fmt(tx - A2)} {fmt(ty)}Z")
    A(f'<path d="{corpo}" fill="url(#lgMetal)"></path>')  # o contorno vem depois das faixas
    # divisões entre os discos: faixa presa ao corpo, com a curvatura do metal,
    # sombra do disco de cima e borda clara do disco de baixo
    id_corpo = "lgCorpoS" if simples else "lgCorpoC"
    A(f'<defs><clipPath id="{id_corpo}"><path d="{corpo}"></path></clipPath>'
      '<linearGradient id="lgVolCil" x1="0" y1="0" x2="1" y2="0"><stop offset="0" style="stop-color:#000000;stop-opacity:.45"></stop>'
      '<stop offset=".28" style="stop-color:#ffffff;stop-opacity:.3"></stop><stop offset=".55" style="stop-color:#000000;stop-opacity:.05"></stop>'
      '<stop offset="1" style="stop-color:#000000;stop-opacity:.5"></stop></linearGradient></defs>')
    esp = A2 * 0.16
    A(f'<g style="clip-path:url(#{id_corpo})">')
    for f_ in faixas:
        fx, fy = T(f_)

        def arco(dy):
            return f"M{fmt(fx - A2)} {fmt(fy + dy)} A{fmt(A2)} {fmt(B2)} 0 0 0 {fmt(fx + A2)} {fmt(fy + dy)}"

        # área entre dois arcos deslocados só na vertical: é o recorte exato de uma faixa
        # em volta do cilindro (um traço grosso no arco cria "orelhas" junto às laterais)
        yt, yb = fy - esp / 2, fy + esp / 2
        faixa = (f"M{fmt(fx - A2)} {fmt(yt)} A{fmt(A2)} {fmt(B2)} 0 0 0 {fmt(fx + A2)} {fmt(yt)} "
                 f"L{fmt(fx + A2)} {fmt(yb)} A{fmt(A2)} {fmt(B2)} 0 0 1 {fmt(fx - A2)} {fmt(yb)}Z")
        A(f'<path d="{faixa}" fill="#2c6fdc"></path>')
        A(f'<path d="{faixa}" fill="url(#lgVolCil)"></path>')
        A(f'<path d="{arco(-esp / 2 - 1.3)}" fill="none" stroke="#000000" style="stroke-opacity:.5;stroke-width:2.6"></path>')
        A(f'<path d="{arco(esp / 2 + 1.2)}" fill="none" stroke="#ffffff" style="stroke-opacity:.6;stroke-width:2.4"></path>')
    A("</g>")
    # contorno aberto em cima: a borda de cima é o traço da tampa, desenhada depois
    contorno_corpo = (f"M{fmt(tx - A2)} {fmt(ty)} L{fmt(tx - A2)} {fmt(by)} A{fmt(A2)} {fmt(B2)} 0 0 0 {fmt(tx + A2)} {fmt(by)} "
                      f"L{fmt(tx + A2)} {fmt(ty)}")
    A(f'<path d="{contorno_corpo}" fill="none" {contorno(".4")}></path>')
    A(f'<path d="M{fmt(tx - A2 * 0.55)} {fmt(ty + B2 + 8)} L{fmt(tx - A2 * 0.55)} {fmt(by - 6)}" stroke="#ffffff" style="stroke-opacity:.45;stroke-width:{fmt(A2 * 0.13)};stroke-linecap:round"></path>')
    for cls, cor in (("luzA", "#d9ecff"), ("luzB", "#fff1cf")):
        A(f'<path class="{cls}" d="M{fmt(tx - A2 + 2)} {fmt(ty + 8)} L{fmt(tx - A2 + 2)} {fmt(by)} M{fmt(tx + A2 - 2)} {fmt(ty + 8)} L{fmt(tx + A2 - 2)} {fmt(by)}" stroke="{cor}" style="stroke-opacity:.8;stroke-width:3"></path>')
    A(f'<ellipse cx="{fmt(tx)}" cy="{fmt(ty)}" rx="{fmt(A2)}" ry="{fmt(B2)}" fill="url(#lgTampa)" {contorno(".35")}></ellipse>')
    A(f'<ellipse cx="{fmt(tx - A2 * 0.1)}" cy="{fmt(ty - B2 * 0.2)}" rx="{fmt(A2 * 0.56)}" ry="{fmt(B2 * 0.43)}" fill="#ffffff" style="fill-opacity:.4"></ellipse>')

    A("@@camada:lupa")
    # lupa: aro de metal, vidro translúcido e cabo escuro que entra na caixa
    if lupa:
        C_ = T(lupa["centro"])
        Rr = lupa["raio"] * escala
        aro = Rr * 0.17
        dd = (math.cos(lupa["ang_cabo"]), math.sin(lupa["ang_cabo"]))
        pp = (-dd[1], dd[0])

        def lu(t, k):
            return (C_[0] + dd[0] * t + pp[0] * k, C_[1] + dd[1] * t + pp[1] * k)

        # disco inclinado para o lado: elipse girada; o aro tem espessura (o lado de trás aparece
        # deslocado), o cabo é um cilindro com sombreado de lado a lado e anel de metal
        rot = lupa["rot"]
        rx_, ry_ = Rr, Rr * 0.8
        cr, sr = math.cos(math.radians(rot)), math.sin(math.radians(rot))

        def el(t, esc=1.0, desl=(0.0, 0.0)):
            x, y = rx_ * esc * math.cos(t), ry_ * esc * math.sin(t)
            return (C_[0] + desl[0] + x * cr - y * sr, C_[1] + desl[1] + x * sr + y * cr)

        prof = (-Rr * 0.12, Rr * 0.07)  # espessura do aro: com a lupa virada para a direita, aparece à esquerda

        def elipse(esc, desl, extra):
            cx_, cy_ = C_[0] + desl[0], C_[1] + desl[1]
            return (f'<ellipse cx="{fmt(cx_)}" cy="{fmt(cy_)}" rx="{fmt(rx_ * esc)}" ry="{fmt(ry_ * esc)}" '
                    f'transform="rotate({fmt(rot)} {fmt(cx_)} {fmt(cy_)})" {extra}></ellipse>')

        # cabo: sai da borda do aro na direção de baixo à direita e entra na caixa
        t_cabo = lupa["ang_cabo"] - math.radians(rot)
        base_cabo = el(t_cabo, 1.0 + aro / Rr * 0.4)
        ang_c = math.atan2(base_cabo[1] - C_[1], base_cabo[0] - C_[0])
        dc = (math.cos(ang_c), math.sin(ang_c))
        pc = (-dc[1], dc[0])

        def cb(t, k):
            return (base_cabo[0] + dc[0] * t + pc[0] * k, base_cabo[1] + dc[1] * t + pc[1] * k)

        wc, wh = Rr * 0.3, Rr * 0.42
        t1, t2 = 13 * escala, 100 * escala
        g1a, g1b = cb(t1, -wh / 2), cb(t1, wh / 2)
        g2a, g2b = cb(0, -wc / 2), cb(0, wc / 2)
        A('<defs>'
          f'<linearGradient id="lgCabo" gradientUnits="userSpaceOnUse" x1="{fmt(g1a[0])}" y1="{fmt(g1a[1])}" x2="{fmt(g1b[0])}" y2="{fmt(g1b[1])}">'
          '<stop offset="0" style="stop-color:#1f130a"></stop><stop offset=".3" style="stop-color:#8a6040"></stop>'
          '<stop offset=".62" style="stop-color:#4a2f1a"></stop><stop offset="1" style="stop-color:#1a0f07"></stop></linearGradient>'
          f'<linearGradient id="lgAnel" gradientUnits="userSpaceOnUse" x1="{fmt(g2a[0])}" y1="{fmt(g2a[1])}" x2="{fmt(g2b[0])}" y2="{fmt(g2b[1])}">'
          '<stop offset="0" style="stop-color:#4b5462"></stop><stop offset=".3" style="stop-color:#dfe5ec"></stop>'
          '<stop offset=".6" style="stop-color:#8c96a6"></stop><stop offset="1" style="stop-color:#3a414d"></stop></linearGradient>'
          '<radialGradient id="lgVidro" cx=".38" cy=".32" r=".75"><stop offset="0" style="stop-color:#ffffff;stop-opacity:.34"></stop>'
          '<stop offset=".65" style="stop-color:#fff4d6;stop-opacity:.12"></stop><stop offset="1" style="stop-color:#f2a93b;stop-opacity:.26"></stop></radialGradient>'
          '</defs>')
        A(f'<path d="{poli([cb(t1, -wh / 2), cb(t1, wh / 2), cb(t2, wh * .46), cb(t2, -wh * .46)])}" fill="url(#lgCabo)" {contorno(".45")}></path>')
        A(f'<path d="{linha(cb(t1 + 5, -wh * .2), cb(t2 - 4, -wh * .18))}" stroke="#ffffff" style="stroke-opacity:.28;stroke-width:{fmt(wh * .13)};stroke-linecap:round"></path>')
        A(f'<path d="{poli([cb(-4, -wc / 2), cb(-4, wc / 2), cb(t1, wc / 2 + 2), cb(t1, -wc / 2 - 2)])}" fill="url(#lgAnel)" {contorno(".45")}></path>')
        A(f'<path d="{linha(cb(t1 * .5, -wc / 2 - 1), cb(t1 * .5, wc / 2 + 1))}" stroke="#000000" style="stroke-opacity:.3;stroke-width:1.6"></path>')
        # aro: camadas de trás para a frente formam a espessura do metal
        for i in range(5):
            f = 1 - i / 4
            tom = ("#4f5764", "#5d6673", "#6d7784", "#7d8794", "#8d97a4")[i]
            A(elipse(1.0, (prof[0] * f, prof[1] * f), f'fill="none" stroke="{tom}" style="stroke-width:{fmt(aro)}"'))
        A(elipse(1.0 + aro / Rr / 2, prof, f'fill="none" {contorno(".4")}'))
        # vidro, com um leve escurecido na borda de baixo (refração) e reflexos
        A(elipse(1.0, (0, 0), 'fill="url(#lgVidro)"'))
        A(elipse(0.86, (Rr * .05, Rr * .05), 'fill="none" stroke="#7a4a12" style="stroke-opacity:.18;stroke-width:3"'))
        A(elipse(1.0, (0, 0), f'fill="none" stroke="url(#lgMetal)" style="stroke-width:{fmt(aro)}"'))
        A(elipse(1.0 + aro / Rr / 2, (0, 0), f'fill="none" {contorno(".45")}'))
        A(elipse(1.0 - aro / Rr / 2, (0, 0), f'fill="none" {contorno(".3", "1.8")}'))
        a0, a1 = math.radians(195), math.radians(262)
        p0, p1 = el(a0, 0.7), el(a1, 0.7)
        A(f'<path d="M{fmt(p0[0])} {fmt(p0[1])} A{fmt(rx_ * .7)} {fmt(ry_ * .7)} {fmt(rot)} 0 1 {fmt(p1[0])} {fmt(p1[1])}" '
          f'fill="none" stroke="#ffffff" style="stroke-opacity:.8;stroke-width:{fmt(Rr * .1)};stroke-linecap:round"></path>')
        brilho = el(math.radians(40), 0.55)
        A(f'<circle cx="{fmt(brilho[0])}" cy="{fmt(brilho[1])}" r="{fmt(Rr * .065)}" fill="#ffffff" style="fill-opacity:.55"></circle>')

    A("@@camada:frente da caixa")
    # frente da caixa
    A(f'<path d="{P(faceE)}" fill="url(#lgFaceE)" {contorno()}></path>')
    A(f'<path d="{P(faceD)}" fill="url(#lgFaceD)" {contorno()}></path>')
    for aba, grad in ((abaFE, "lgAbaFE"), (abaFD, "lgAbaFD")):
        A(f'<path d="{P(aba)}" fill="url(#{grad})" {contorno()}></path>')
    # bordas cortadas do papelão das abas da frente (as de trás vão junto com as próprias abas,
    # senão o realce passa por cima do lápis e da planilha) e dobras do aro
    bordas = [linha(T(aba[2]), T(aba[3])) for aba in (abaFE, abaFD)]
    A(f'<path d="{" ".join(bordas)}" stroke="#fff3dc" style="stroke-opacity:.75;stroke-width:3;stroke-linecap:round"></path>')
    lt, ft, rt = T(L), T(F), T(R)
    A(f'<path d="M{fmt(lt[0])} {fmt(lt[1])} L{fmt(ft[0])} {fmt(ft[1])} L{fmt(rt[0])} {fmt(rt[1])}" fill="none" stroke="#ffffff" style="stroke-opacity:.55;stroke-width:2.6;stroke-linejoin:round"></path>')
    f0t, ftt = T(F0), T(F)
    A(f'<path d="{linha(ftt, f0t)}" stroke="#ffffff" style="stroke-opacity:.35;stroke-width:2.4"></path>')
    A("</g>")
    if bruto:
        return out
    return "\n".join(x for x in out if not x.startswith("@@camada:"))


DEFS = """<linearGradient id="lgAbaTE" x1="1" y1="1" x2="0" y2="0"><stop offset="0" style="stop-color:#f0c48a"></stop><stop offset="1" style="stop-color:#c9904f"></stop></linearGradient>
<linearGradient id="lgAbaTD" x1="0" y1="1" x2="1" y2="0"><stop offset="0" style="stop-color:#e3b377"></stop><stop offset="1" style="stop-color:#b97f41"></stop></linearGradient>
<linearGradient id="lgAbaFE" x1="0" y1="0" x2="0" y2="1"><stop offset="0" style="stop-color:#e6b678"></stop><stop offset="1" style="stop-color:#c88d4e"></stop></linearGradient>
<linearGradient id="lgAbaFD" x1="0" y1="0" x2="0" y2="1"><stop offset="0" style="stop-color:#cf975a"></stop><stop offset="1" style="stop-color:#aa713a"></stop></linearGradient>
<linearGradient id="lgFaceE" x1="0" y1="0" x2="0" y2="1"><stop offset="0" style="stop-color:#d39a5c"></stop><stop offset="1" style="stop-color:#ad7239"></stop></linearGradient>
<linearGradient id="lgFaceD" x1="0" y1="0" x2="0" y2="1"><stop offset="0" style="stop-color:#ad7238"></stop><stop offset="1" style="stop-color:#8a5624"></stop></linearGradient>
<linearGradient id="lgParedeE" x1="0" y1="0" x2="1" y2="1"><stop offset="0" style="stop-color:#8d5d2e"></stop><stop offset="1" style="stop-color:#4b2e13"></stop></linearGradient>
<linearGradient id="lgParedeD" x1="1" y1="0" x2="0" y2="1"><stop offset="0" style="stop-color:#7b4e25"></stop><stop offset="1" style="stop-color:#422710"></stop></linearGradient>
<linearGradient id="lgMetal" x1="0" y1="0" x2="1" y2="0"><stop offset="0" style="stop-color:#4b5462"></stop><stop offset=".28" style="stop-color:#c9d2de"></stop><stop offset=".55" style="stop-color:#8c96a6"></stop><stop offset="1" style="stop-color:#424a57"></stop></linearGradient>
<radialGradient id="lgTampa" cx=".45" cy=".35" r=".8"><stop offset="0" style="stop-color:#f3f6f9"></stop><stop offset="1" style="stop-color:#97a1b0"></stop></radialGradient>
<linearGradient id="lgPapel" x1="0" y1="0" x2="0" y2="1"><stop offset="0" style="stop-color:#ffffff"></stop><stop offset="1" style="stop-color:#d6dbe3"></stop></linearGradient>
<linearGradient id="lgRealce" x1="0" y1="0" x2="0" y2="1"><stop offset="0" style="stop-color:#ffffff;stop-opacity:.6"></stop><stop offset="1" style="stop-color:#ffffff;stop-opacity:0"></stop></linearGradient>
<radialGradient id="lgLuzA" cx=".5" cy=".5" r=".5"><stop offset="0" style="stop-color:#f2f8ff;stop-opacity:.95"></stop><stop offset=".24" style="stop-color:#a8d0ff;stop-opacity:.65"></stop><stop offset=".6" style="stop-color:#3f86ee;stop-opacity:.2"></stop><stop offset="1" style="stop-color:#2c6fdc;stop-opacity:0"></stop></radialGradient>
<radialGradient id="lgLuzB" cx=".5" cy=".5" r=".5"><stop offset="0" style="stop-color:#fffbea;stop-opacity:.95"></stop><stop offset=".24" style="stop-color:#ffe39a;stop-opacity:.68"></stop><stop offset=".6" style="stop-color:#f2a93b;stop-opacity:.2"></stop><stop offset="1" style="stop-color:#e08a1e;stop-opacity:0"></stop></radialGradient>
<radialGradient id="lgDentroA" cx=".5" cy=".4" r=".6"><stop offset="0" style="stop-color:#cfe6ff;stop-opacity:.55"></stop><stop offset="1" style="stop-color:#2c6fdc;stop-opacity:0"></stop></radialGradient>
<radialGradient id="lgDentroB" cx=".5" cy=".4" r=".6"><stop offset="0" style="stop-color:#fff1c4;stop-opacity:.6"></stop><stop offset="1" style="stop-color:#f2a93b;stop-opacity:0"></stop></radialGradient>
<linearGradient id="lgRaioA" x1="0" y1="1" x2="0" y2="0"><stop offset="0" style="stop-color:#e6f2ff;stop-opacity:.45"></stop><stop offset="1" style="stop-color:#e6f2ff;stop-opacity:0"></stop></linearGradient>
<linearGradient id="lgRaioB" x1="0" y1="1" x2="0" y2="0"><stop offset="0" style="stop-color:#fff4d6;stop-opacity:.45"></stop><stop offset="1" style="stop-color:#fff4d6;stop-opacity:0"></stop></linearGradient>
<filter id="lgSombra" x="-15%" y="-15%" width="130%" height="135%"><feDropShadow dx="0" dy="10" stdDeviation="10" style="flood-color:#000000;flood-opacity:.4"></feDropShadow></filter>"""


def svg(frag, holes):
    return (f'<svg class="logo {holes}" viewBox="0 0 512 512" width="{{{{z.s}}}}" height="{{{{z.s}}}}" '
            f'aria-hidden="true">{frag}</svg>')


def prancheta(completo):
    """A prancheta 10 (Logo.dc.html): só o logo completo, em todos os tamanhos (decisão de 17/10/2026)."""
    estilo = """body{margin:0;background:#0e0f11}
.est{--acc:#2c6fdc;--fundo0:#111214;--painel:#1e1f22;--c:#191a1d;--p:#212226;--h:rgba(255,255,255,.07);--b:rgba(255,255,255,.07);--b2:rgba(255,255,255,.12);--t:#ececee;--t2:#b9bbc0;--t3:#9597a0;--vidrob:rgba(255,255,255,.09);--vidroluz:rgba(255,255,255,.06);--acct:color-mix(in oklab,var(--acc) 58%,white);--bt1:#3b3c40;--bt2:#26272a;--btb:rgba(0,0,0,.6);--btluz:rgba(255,255,255,.13);--textura:repeating-linear-gradient(135deg,rgba(255,255,255,.022) 0 1px,transparent 1px 3px);font-family:"Segoe UI Variable Text","Segoe UI",system-ui,sans-serif;font-size:12px;line-height:1.4;color:var(--t);background:var(--fundo0)}
.est *{box-sizing:border-box}
.painel{background:var(--painel);border:1px solid var(--vidrob);border-radius:14px;box-shadow:inset 0 1px 0 var(--vidroluz),0 1px 3px rgba(0,0,0,.2);overflow:hidden;display:flex;flex-direction:column}
.cab{height:38px;flex:none;display:flex;align-items:center;gap:8px;padding:0 14px;border-bottom:1px solid var(--btb);background:var(--textura),linear-gradient(to bottom,var(--bt1),var(--bt2));box-shadow:inset 0 1px 0 var(--btluz)}
.tit{font-size:11px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;color:var(--t3)}
.logo{display:block;flex:none}
.logo.vA .luzB,.logo.vB .luzA{display:none}
.palco{flex:1;display:flex;align-items:center;justify-content:center;gap:28px;padding:18px}
.claro-fundo{background:#e3e5e9;color:#1b1c1f}
.escada{display:flex;align-items:flex-end;gap:26px;padding:18px 22px}
.escada figure{margin:0;display:flex;flex-direction:column;align-items:center;gap:8px}
.escada figcaption{font-size:11px;color:var(--t3);font-variant-numeric:tabular-nums}
.claro-fundo figcaption{color:#62656c}
.marca{display:flex;flex-direction:column;gap:2px}
.marca b{font-size:28px;font-weight:600;letter-spacing:-.01em;line-height:1.1}
.marca span{font-size:13px;font-weight:350;color:var(--t2)}
.claro-fundo .marca span{color:#45474d}"""
    bloco = svg(completo, "{{z.v}}")

    def laco(lista, conteudo, n):
        return f'<sc-for list="{{{{{lista}}}}}" as="z" hint-placeholder-count="{n}">{conteudo}</sc-for>'

    figura = f'<figure>{bloco}<figcaption>{{{{z.rot}}}}</figcaption></figure>'
    corpo = f"""<div class="est" style="width:100%;min-height:1180px;display:flex;flex-direction:column;gap:16px;padding:24px 28px 32px">
<svg width="0" height="0" style="position:absolute" aria-hidden="true" focusable="false"><defs>
{DEFS}
</defs></svg>

<div>
<h1 style="margin:0;font-size:20px;font-weight:600">Logo do aplicativo</h1>
<p style="margin:4px 0 0;color:var(--t2);max-width:820px">Caixa de papelão aberta, em isométrico, com um banco e uma planilha saindo dela, uma lupa na frente e um lápis no fundo, e luz âmbar por trás. Mesmo acabamento dos ícones: cilindro metálico com faixas de acento, folha com cabeçalho, contornos escuros suaves e sombra curta. É a única versão do logo: o mesmo desenho completo em todos os tamanhos, dos 16 px da barra de título aos 380 px desta prancheta.</p>
</div>

<div style="display:flex;gap:16px;height:470px">
<section class="painel" style="flex:1" aria-label="Logo completo">
<div class="cab"><span class="tit">Completo · luz âmbar</span></div>
<div class="palco">{laco("heroiA", bloco, 1)}</div>
</section>
<section class="painel" style="flex:1" aria-label="Tamanhos pequenos">
<div class="cab"><span class="tit">Pequenos · tamanho real</span></div>
<div class="palco" style="flex-direction:column;gap:18px"><div class="escada" style="padding:0">{laco("pequenos", figura, 4)}</div><p style="margin:0;max-width:300px;text-align:center;color:var(--t2)">O mesmo logo completo, sem versão simplificada, também a 32, 24, 20 e 16 px.</p></div>
</section>
<section class="painel" style="flex:1" aria-label="Fundo claro com o nome">
<div class="cab"><span class="tit">Fundo claro · com o nome</span></div>
<div class="palco claro-fundo" style="flex-direction:column;gap:14px">{laco("heroiClaro", bloco, 1)}<div class="marca" style="align-items:center"><b>DataBox</b><span>bancos, planilhas e textos de dados</span></div></div>
</section>
</div>

<section class="painel" aria-label="Tamanhos">
<div class="cab"><span class="tit">Tamanhos</span><span style="color:var(--t3);font-size:11px">só o logo completo, em todos os tamanhos</span></div>
<div style="display:flex;flex-wrap:wrap">
<div class="escada" style="flex:1">{laco("escada", figura, 7)}</div>
<div class="escada claro-fundo" style="flex:1">{laco("escada", figura, 7)}</div>
</div>
</section>

<section class="painel" aria-label="Em uso" style="width: 1386px">
<div class="cab"><span class="tit">Em uso</span></div>
<div style="display:flex;gap:16px;padding:16px;flex-wrap:wrap;align-items:flex-start">
<div style="width:620px;border:1px solid var(--b2);border-radius:10px;overflow:hidden">
<div style="height:30px;display:flex;align-items:center;position:relative;background:#131416;color:var(--t2)"><span style="width:36px;display:flex;justify-content:center">{laco("barra", bloco, 1)}</span><span style="display:flex;gap:16px;padding:0 8px">Arquivo<span>Editar</span><span>Exibir</span><span>Ajuda</span></span><span style="flex:1"></span><span style="position:absolute;left:50%;transform:translateX(-50%);color:var(--t3)">dados · DataBox</span><span style="width:46px;text-align:center">—</span><span style="width:46px;text-align:center">☐</span><span style="width:46px;text-align:center">✕</span></div>
<div style="height:30px;display:flex;align-items:center;position:relative;background:#e8eaee;color:#45474d"><span style="width:36px;display:flex;justify-content:center">{laco("barra", bloco, 1)}</span><span style="display:flex;gap:16px;padding:0 8px">Arquivo<span>Editar</span><span>Exibir</span><span>Ajuda</span></span><span style="flex:1"></span><span style="position:absolute;left:50%;transform:translateX(-50%);color:#62656c">dados · DataBox</span><span style="width:46px;text-align:center">—</span><span style="width:46px;text-align:center">☐</span><span style="width:46px;text-align:center">✕</span></div>
</div>
<div style="flex:1;min-width:380px;display:flex;align-items:center;gap:18px;padding:16px 20px;border:1px solid var(--b2);border-radius:10px;background:var(--c)">{laco("inicio", bloco, 1)}<div class="marca"><b>DataBox</b><span>Abra uma pasta ou um arquivo para começar</span></div></div>
<div style="display:flex;align-items:center;gap:6px;padding:6px 10px;border-radius:10px;background:#1c1d21;border:1px solid var(--b2)">{laco("tarefas", '<span style="width:44px;height:40px;display:flex;align-items:center;justify-content:center;border-radius:6px;background:rgba(255,255,255,.08);border-bottom:2px solid #6aa2ff">' + bloco + '</span>', 1)}</div>
</div>
</section>
</div>"""
    script = """class Component extends DCLogic {
  renderVals() {
    const um = (s, v, rot) => ({ s: s, v: v, rot: rot || '' });
    return {
      heroiA: [um(380, 'vB')],
      pequenos: [um(32, 'vB', '32'), um(24, 'vB', '24'), um(20, 'vB', '20'), um(16, 'vB', '16')],
      heroiClaro: [um(260, 'vB')],
      escada: [um(128, 'vB', '128'), um(64, 'vB', '64'), um(48, 'vB', '48'), um(32, 'vB', '32'), um(24, 'vB', '24'), um(20, 'vB', '20'), um(16, 'vB', '16')],
      barra: [um(16, 'vB')],
      inicio: [um(72, 'vB')],
      tarefas: [um(32, 'vB')]
    };
  }
}"""
    return f"""<!doctype html>
<html lang="pt-BR">
<head>
<meta charset="utf-8">
<title>Logo do aplicativo</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<style>
{estilo}
</style>
</helmet>
{corpo}
</x-dc>
<script type="text/x-dc" data-dc-script data-props='{{"$preview":{{"width":1440,"height":1180}}}}'>
{script}
</script>
</body>
</html>
"""


if __name__ == "__main__":
    completo = gerar(False)
    simples = gerar(True)  # histórico: a versão simplificada não é mais usada (17/10/2026), mas o fragmento continua gravado
    saida = Path(__file__).resolve().parent.parent / "logo" / "fragmentos"
    saida.mkdir(exist_ok=True)
    (saida / "completo.svgfrag").write_text(completo, encoding="utf-8")
    (saida / "simples.svgfrag").write_text(simples, encoding="utf-8")
    (saida / "defs.svgfrag").write_text(DEFS, encoding="utf-8")
    destino = PROJETO / "Logo.dc.html"
    destino.write_text(prancheta(completo), encoding="utf-8", newline="\n")
    print(destino, destino.stat().st_size, "bytes", file=sys.stderr)
