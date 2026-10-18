"""Só o logo completo (17/10/2026): troca, em cada prancheta, o logo simplificado das cópias pequenas (a da barra de
título, 16 px, e a do cabeçalho do Sobre, 20 px) pelo logo completo, o `logo/fragmentos/final.svgfrag` (o mesmo de
`logo/databox.svg`), no mesmo tamanho. Decisão do Isaías na prancheta 10: a versão simplificada não é mais usada em
nenhum tamanho.

- Cada `<svg class="logo" viewBox="0 0 512 512" width="N" height="N" aria-hidden="true">…</svg>` é lido: o que for
  o fragmento simplificado (ou o simplificado com o sufixo "Sb" do `preferencias.py`) vira o completo (com o mesmo
  sufixo); o completo (com ou sem o sufixo "Sg") já está certo e fica. Qualquer outro conteúdo faz o script parar,
  sem gravar nada.
- O completo usa o filtro `lgSombraPeca`: a prancheta que não o define ganha a definição logo depois do `lgSombra`
  (o mesmo que o `logo_final.py` faz no Início).
- Idempotente: a prancheta sem nenhum logo simplificado é pulada ("já aplicado"). `Logo.dc.html` (prancheta 10) e
  `LogoEditavel.dc.html` não entram: a primeira sai do `logo_final.py` (via `logo.prancheta`), a segunda não tem
  cópias do logo.

Uso: python design/ferramentas/logo_so_completo.py [Prancheta.dc.html …]   (sem argumentos: todas). Só biblioteca
padrão; o simplificado de referência fica em `logo/fragmentos/simples.svgfrag`, só para reconhecer o que trocar."""
import re
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
DESIGN = AQUI.parent
CANVAS = DESIGN / "canvas"
FRAGMENTOS = DESIGN / "logo" / "fragmentos"
sys.stdout.reconfigure(encoding="utf-8")

FORA = {"Logo.dc.html", "LogoEditavel.dc.html"}
LOGO = re.compile(r'<svg class="logo" viewBox="0 0 512 512" width="(\d+)" height="(\d+)" aria-hidden="true">(.*?)</svg>', re.S)
SOMBRA = re.compile(r'<filter id="lgSombra".*?</filter>', re.S)
SOMBRA_PECA = re.compile(r'<filter id="lgSombraPeca".*?</filter>', re.S)


def renomear(svg, sufixo):
    """Dá sufixo às definições locais de um SVG (o mesmo do `preferencias.py`: cópias do logo não repetem ids)."""
    for i in re.findall(r'id="([^"]+)"', svg):
        svg = svg.replace(f'id="{i}"', f'id="{i}{sufixo}"').replace(f"url(#{i})", f"url(#{i}{sufixo})")
    return svg


def ler(nome):
    return (FRAGMENTOS / nome).read_text(encoding="utf-8")


def sombra_peca():
    """A definição do `lgSombraPeca`, tirada do próprio Início (que a ganhou do `logo_final.py`)."""
    texto = (CANVAS / "Inicio.dc.html").read_text(encoding="utf-8")
    achou = SOMBRA_PECA.findall(texto)
    if len(achou) != 1:
        raise SystemExit(f"Inicio.dc.html devia ter uma definição do lgSombraPeca, tem {len(achou)}.")
    return achou[0]


def reconhecer(corpo, simples, completo):
    """('simples' | 'completo', sufixo) do conteúdo de um logo; None se não for nenhum dos conhecidos."""
    for sufixo in ("", "Sb", "Sg"):
        for tipo, frag in (("simples", simples), ("completo", completo)):
            if corpo == (renomear(frag, sufixo) if sufixo else frag):
                return tipo, sufixo
    return None


def planejar(nome, texto, simples, completo):
    """O texto novo da prancheta (ou None se já está certa); recusa o que não conhece."""
    achados = list(LOGO.finditer(texto))
    if not achados:
        print(f"{nome}: sem logo")
        return None
    trocas = []
    for m in achados:
        tipo = reconhecer(m.group(3), simples, completo)
        lado = m.group(1)
        if lado != m.group(2) or tipo is None:
            raise SystemExit(f"{nome}: logo de {lado}×{m.group(2)} px com conteúdo desconhecido; nada foi gravado.")
        if tipo[0] == "simples":
            trocas.append((m, tipo[1]))
    if not trocas:
        print(f"{nome}: já aplicado ({len(achados)} logo(s) completo(s))")
        return None
    novo, ultimo = [], 0
    for m, sufixo in trocas:
        frag = renomear(completo, sufixo) if sufixo else completo
        novo.append(texto[ultimo : m.start(3)])
        novo.append(frag)
        ultimo = m.end(3)
    novo.append(texto[ultimo:])
    texto = "".join(novo)
    if not SOMBRA_PECA.search(texto):
        ancora = SOMBRA.search(texto)
        if ancora is None:
            raise SystemExit(f"{nome}: sem o filtro lgSombra para pôr o lgSombraPeca depois; nada foi gravado.")
        texto = texto[: ancora.end()] + "\n" + sombra_peca() + texto[ancora.end() :]
    print(f"{nome}: {len(trocas)} logo(s) simplificado(s) → completo ({', '.join(m.group(1) + ' px' for m, _ in trocas)})")
    return texto


def main(nomes):
    simples, completo = ler("simples.svgfrag"), ler("final.svgfrag")
    if not nomes:
        nomes = sorted(p.name for p in CANVAS.glob("*.dc.html") if p.name not in FORA)
    novos = {}
    for nome in nomes:
        if nome in FORA:
            raise SystemExit(f"{nome} não é desta ferramenta (veja o começo do arquivo).")
        texto = (CANVAS / nome).read_text(encoding="utf-8")
        novo = planejar(nome, texto, simples, completo)
        if novo is not None:
            novos[nome] = novo
    for nome, texto in novos.items():  # só grava depois de conferir todas
        (CANVAS / nome).write_text(texto, encoding="utf-8", newline="\n")
    print(f"{len(novos)} prancheta(s) gravada(s)")


if __name__ == "__main__":
    main(sys.argv[1:])
