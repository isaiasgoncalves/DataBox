"""O vidro das camadas que se sobrepõem um pouco mais transparente, pedido do Isaías em 17/10/2026 ("pode aumentar
um pouco mais a transparência"): o --flut passa de 88% para 78% de opacidade no escuro e de 90% para 80% no claro,
em todas as pranchetas que o definem (menus, menus de contexto, dicas, janelas e o aviso flutuante usam o .flut).
O desfoque (--desfoque) continua o mesmo. O app segue os mesmos valores (interface\\src\\tema.rs).
Pode rodar de novo: a prancheta que já tem os valores novos fica como está; uma que tenha outro --flut faz parar.
Uso: python -I -X utf8 vidro_transparente.py [pasta das pranchetas]"""
import re
import sys
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8")
PASTA = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent / "canvas"
TROCAS = {
    "--flut:rgba(30,31,35,.88)": "--flut:rgba(30,31,35,.78)",
    "--flut:rgba(255,255,255,.9)": "--flut:rgba(255,255,255,.8)",
}
NOVOS = set(TROCAS.values())

feitas = 0
for arq in sorted(PASTA.glob("*.dc.html")):
    t = arq.read_text(encoding="utf-8")
    achados = re.findall(r"--flut:rgba\([^)]*\)", t)
    if not achados:
        continue
    desconhecidos = [a for a in achados if a not in TROCAS and a not in NOVOS]
    assert not desconhecidos, f"{arq.name}: --flut inesperado {desconhecidos}"
    if all(a in NOVOS for a in achados):
        print(arq.name, "já tem")
        continue
    for antigo, novo in TROCAS.items():
        t = t.replace(antigo, novo)
    arq.write_text(t, encoding="utf-8", newline="\n")
    print(arq.name, "vidro mais transparente")
    feitas += 1
print(feitas, "pranchetas alteradas")
