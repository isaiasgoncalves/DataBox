"""Cabeçalho dos painéis (.cab: Explorador, Propriedades, Histórico e as seções da biblioteca), pedido do Isaías em
16/10/2026: 0,6 da altura de antes, de 38 para 23 px, como padrão em todas as telas. Os botões de ícone dentro dele
(Abrir pasta, Recolher tudo…) encolhem para 20 px, com a silhueta de 12. As pranchetas do logo ficam como estão.
Uso: python cabecalho_fino.py [pasta das pranchetas]"""
import sys
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8")
PASTA = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent / "canvas"
MARCA = "/* cabeçalho fino de 16/10/2026 */"
CSS = (MARCA + "\n.cab{height:23px;padding:0 4px 0 12px}"
       ".cab .ib,.cab .ib.pq{width:20px;height:20px}.cab .ib .sil,.cab .ib .ic{width:12px;height:12px}"
       ".cab .tit{font-size:10.5px;line-height:1}\n")
FORA = {"Logo.dc.html", "LogoEditavel.dc.html"}

for arq in sorted(PASTA.glob("*.dc.html")):
    if arq.name in FORA:
        continue
    t = arq.read_text(encoding="utf-8")
    if ".cab{" not in t[: t.find("</helmet>")]:
        continue
    if MARCA in t:
        print(arq.name, "já tem")
        continue
    assert t.count("</style>\n</helmet>") == 1, arq.name
    t = t.replace("</style>\n</helmet>", CSS + "</style>\n</helmet>", 1)
    arq.write_text(t, encoding="utf-8", newline="\n")
    print(arq.name, "cabeçalho de 23 px")
