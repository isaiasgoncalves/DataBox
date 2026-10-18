"""Resolve os color-mix(in oklab, …) dos tokens das pranchetas para RGBA fixos, como o navegador faz
(interpolação pré-multiplicada em OKLab), e imprime as constantes para o tema.rs.

Uso: python tokens_oklab.py"""
import math

ACENTOS = {"azul": "#2c6fdc", "verde": "#0d7f74", "violeta": "#6b4fd8", "laranja": "#c2540a"}


def hex_rgba(h, a=1.0):
    h = h.lstrip("#")
    return (int(h[0:2], 16) / 255, int(h[2:4], 16) / 255, int(h[4:6], 16) / 255, a)


def lin(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def gam(c):
    return 12.92 * c if c <= 0.0031308 else 1.055 * c ** (1 / 2.4) - 0.055


def para_oklab(rgb):
    r, g, b = (lin(x) for x in rgb)
    l_ = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b
    m_ = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b
    s_ = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b
    l_, m_, s_ = (math.copysign(abs(v) ** (1 / 3), v) for v in (l_, m_, s_))
    return (0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
            1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
            0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_)


def de_oklab(lab):
    L, a, b = lab
    l_ = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3
    m_ = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3
    s_ = (L - 0.0894841775 * a - 1.2914855480 * b) ** 3
    r = 4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_
    g = -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_
    bb = -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_
    return tuple(min(1.0, max(0.0, gam(x))) for x in (r, g, bb))


def mix(c1, p1, c2):
    """color-mix(in oklab, c1 p1, c2) com alfas (interpolação pré-multiplicada, como no CSS Color 4)."""
    if c2 == "transparent":
        return (c1[0], c1[1], c1[2], c1[3] * p1)
    a = c1[3] * p1 + c2[3] * (1 - p1)
    if a == 0:
        return (0, 0, 0, 0)
    l1, l2 = para_oklab(c1[:3]), para_oklab(c2[:3])
    lab = tuple((x * c1[3] * p1 + y * c2[3] * (1 - p1)) / a for x, y in zip(l1, l2))
    return (*de_oklab(lab), a)


def rust(nome, c):
    r, g, b, a = c
    r, g, b = (round(x * 255) for x in (r, g, b))
    if abs(a - 1) < 1e-9:
        return f"{nome}: rgb(0x{r:02X}, 0x{g:02X}, 0x{b:02X}),"
    return f"{nome}: rgba({r}, {g}, {b}, {round(a, 3)}),"


BRANCO, PRETO = hex_rgba("#ffffff"), hex_rgba("#000000")

if __name__ == "__main__":
    for tema, (bt1, bt2, fbc, alfas, acct_mix) in {
        "ESCURA": ("#3b3c40", "#26272a", (255, 255, 255, .40), (.30, .12, .20, .60, .78), (.58, BRANCO)),
        "CLARA": ("#ffffff", "#e6e7ea", (0, 0, 0, .46), (.17, .07, .11, .45, .62), (.85, PRETO)),
    }.items():
        print(f"// {tema}")
        print(rust("sup_meio", mix(hex_rgba(bt1), .5, hex_rgba(bt2))))
        for nome_ac, hx in ACENTOS.items():
            acc = hex_rgba(hx)
            print(f"//   destaque {nome_ac}")
            print(rust("  acc", acc))
            print(rust("  acct", mix(acc, acct_mix[0], acct_mix[1])))
            for nome, al in zip(("sel", "selh", "tint", "brilho", "brilho_forte"), alfas):
                print(rust("  " + nome, mix(acc, al, "transparent")))
            print(rust("  pri_topo", mix(acc, .72, BRANCO)))
            print(rust("  pri_base", mix(acc, .80, PRETO)))
            print(rust("  borda_chip", mix(acc, .45, "transparent")))
            # o hover da borda dos campos (`--fbc`, revisão de 15/10/2026, item 2; antes era sobre o `--fb`)
            print(rust("  fbc_hover", mix(acc, .45, (fbc[0] / 255, fbc[1] / 255, fbc[2] / 255, fbc[3]))))
            print(rust("  brilho_item", mix(acc, .22, "transparent")))
