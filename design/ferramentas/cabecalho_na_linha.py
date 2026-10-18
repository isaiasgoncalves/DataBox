"""Cabeçalho dos painéis na largura da linha da árvore (.cab: Explorador, Propriedades, Histórico e as seções da
biblioteca), pedido do Isaías em 17/10/2026: "mudar a largura do cabeçalho para ser a mesma da barrinha azul do
seletor". O .cab deixa de ir de borda a borda: ganha a margem do .no (6px dos lados, mais a borda de 1px do painel
= 7px da beirada), a mesma distância em cima, os quatro cantos de 6px (o raio do .no) e o fio --btb de 1px em volta
toda, no lugar do fio de baixo. A altura continua 23px (border-box). O corpo do painel começa logo embaixo da faixa;
na árvore, o padding de 6px de cima deixa o vão até a primeira linha igual ao dos lados.
Roda depois do cabecalho_fino.py (recusa a prancheta que ainda não tem a regra dele). As pranchetas do logo ficam
como estão. Pode rodar de novo: a marca evita repetir.
Uso: python -I -X utf8 cabecalho_na_linha.py [pasta das pranchetas]"""
import sys
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8")
PASTA = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent / "canvas"
ANTES = "/* cabeçalho fino de 16/10/2026 */"
MARCA = "/* cabeçalho na largura da linha de 17/10/2026 */"
CSS = MARCA + "\n.cab{margin:6px 6px 0;border:1px solid var(--btb);border-radius:6px}\n"
FIM = "</style>\n</helmet>"
FORA = {"Logo.dc.html", "LogoEditavel.dc.html"}

feitas = 0
for arq in sorted(PASTA.glob("*.dc.html")):
    if arq.name in FORA:
        continue
    t = arq.read_text(encoding="utf-8")
    helmet = t[: t.find("</helmet>")]
    if ".cab{" not in helmet:
        continue
    if MARCA in t:
        print(arq.name, "já tem")
        continue
    # recusa o que não conhece: sem a regra de 23px não dá para saber o que o .cab tem agora
    assert ANTES in t, f"{arq.name}: rode o cabecalho_fino.py antes"
    assert t.count(FIM) == 1, f"{arq.name}: fim da folha de estilo não encontrado"
    # a nova regra vai no fim e tem de vir depois de todas as outras que mexem na borda do .cab: a do 23px é a última
    assert helmet.rfind(".cab{") > helmet.index(ANTES), f"{arq.name}: ordem das regras do .cab inesperada"
    t = t.replace(FIM, CSS + FIM, 1)
    arq.write_text(t, encoding="utf-8", newline="\n")
    print(arq.name, "cabeçalho na largura da linha")
    feitas += 1
print(feitas, "pranchetas alteradas")
