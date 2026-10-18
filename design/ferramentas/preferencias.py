"""Prancheta 13: Preferências (Arquivo > Preferências, Ctrl+,) na seção Aparência — tema, cores do código e fonte do
código — e a janela Sobre o DataBox (Ajuda > Sobre o DataBox), com o logo, a versão e as pastas do aplicativo. Pedidas
pelo Isaías em 15/10/2026. Mesmo sistema das outras pranchetas: folha de estilo e fundo do Exportar.dc.html, ícones e
logo do Inicio.dc.html. Gera ../canvas/Preferencias.dc.html; refaz a prancheta inteira.

Revisão de 15/10/2026, aprovada em 16/10/2026, aplicada aqui:
- 13: os ícones de 20 px com o contorno de até 24 px (corpo 2,4/.35, dobra 1,6/.25); os desenhos de 48 ficam iguais;
- 16: a fonte do código em 12,5 px, a mesma dos editores;
- 22: sem a Consolas no Sobre (ela fica só dentro dos editores) e "Arquivo de log" no lugar de "Registro";
- D3: as preferências valem na hora; Salvar fica desligado até algo mudar (é o estado desenhado), Cancelar, Esc e o
  × voltam ao estado de quando a janela abriu, e "Restaurar o padrão" vale só para a seção visível (dito na dica).
Rode antes os geradores revisao_*.py, que mudam o Exportar e o Início de onde esta prancheta tira as peças.
Uso: python preferencias.py [pasta das pranchetas]"""
import math
import re
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
PASTA = Path(sys.argv[1]) if len(sys.argv) > 1 else AQUI.parent / "canvas"
sys.stdout.reconfigure(encoding="utf-8")

base = (PASTA / "Exportar.dc.html").read_text(encoding="utf-8")
inicio = (PASTA / "Inicio.dc.html").read_text(encoding="utf-8")
cabeca = base[: base.index("</helmet>")]
estilo_extra = """.est{--sel:color-mix(in oklab,var(--acc) 30%,transparent)}
.est.claro{--sel:color-mix(in oklab,var(--acc) 17%,transparent)}
.no{height:24px;display:flex;align-items:center;gap:6px;padding-right:10px;color:var(--t2);text-decoration:none;border-radius:6px;margin:0 6px;white-space:nowrap}
.no:hover{background:var(--h);color:var(--t)}
.no.on{background:var(--sel);color:var(--t);box-shadow:inset 0 1px 0 rgba(255,255,255,.07),0 0 10px color-mix(in oklab,var(--acc) 22%,transparent)}
.btn.sutil{background:transparent;border-color:transparent;box-shadow:none;color:var(--t2)}
.dlg.pref{width:760px}
.pref-corpo{display:grid;grid-template-columns:184px minmax(0,1fr);min-height:432px}
.pref-nav{display:flex;flex-direction:column;gap:2px;padding:12px 0;border-right:1px solid var(--vidrob)}
.pref-nav .no{height:32px;padding:0 12px 0 9px;margin:0 8px;color:var(--t2);gap:10px}
.pref-nav .sk{display:flex}
.pref-nav .sk.arq .rot{font-size:6.6px;letter-spacing:0}
.pref-nav .no.on{color:var(--t);font-weight:600}
.pref-sec{padding:16px 22px 18px;display:flex;flex-direction:column;min-width:0}
.pref-sec h3{margin:0 0 2px;font-size:14px;font-weight:600;color:var(--t)}
.pref-l{display:grid;grid-template-columns:140px minmax(0,1fr);gap:0 18px;align-items:center;padding:14px 0;border-bottom:1px solid var(--b)}
.pref-l:last-child{border-bottom:0}
.pref-l.alto{align-items:start}
.pref-r{display:flex;flex-direction:column;gap:3px;min-width:0}
.pref-r b{font-weight:600;color:var(--t)}
.pref-r span{font-size:11px;color:var(--t3);line-height:15px}
.pref-l .seg{display:grid;width:100%}
.pref-l .seg button{padding:0 10px}
.pref-fonte{display:flex;gap:8px}
.pref-fonte .campo{display:flex}
.cc{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:10px}
.cc-c{display:flex;flex-direction:column;gap:7px;padding:6px 6px 8px;border-radius:10px;border:1px solid var(--fb);background:var(--f);color:var(--t);font:inherit;text-align:left;cursor:pointer;min-width:0;transition:border-color .2s ease,box-shadow .2s ease}
.cc-c:hover{border-color:color-mix(in oklab,var(--acc) 45%,var(--fb))}
.cc-c.on{border-color:var(--acc);box-shadow:0 0 0 1px var(--acc),0 0 14px var(--brilho)}
.cc-p{display:block;border-radius:6px;padding:7px 9px;font-family:Consolas,"Cascadia Mono",monospace;font-size:11px;line-height:16px;white-space:pre;overflow:hidden;box-shadow:inset 0 0 0 1px rgba(0,0,0,.28),inset 0 1px 2px rgba(0,0,0,.3)}
.cc-n{display:flex;align-items:baseline;gap:6px;padding:0 3px;font-weight:600;white-space:nowrap;overflow:hidden}
.cc-n small{font-weight:400;color:var(--t3);font-size:11px;overflow:hidden;text-overflow:ellipsis}
.dlg.sobre{width:440px}
.sb-corpo{padding:22px 24px 20px;display:flex;flex-direction:column;gap:18px}
.sb-topo{display:flex;align-items:center;gap:18px}
.sb-id{display:flex;flex-direction:column;gap:3px;min-width:0}
.sb-nome{font-size:20px;font-weight:600;color:var(--t);letter-spacing:-.01em}
.sb-ver{color:var(--t2);font-variant-numeric:tabular-nums}
.sb-desc{color:var(--t3);font-size:11px;line-height:15px;margin-top:3px}
.sb-det{display:grid;grid-template-columns:92px minmax(0,1fr);gap:7px 14px;margin:0;padding:14px 0;border-top:1px solid var(--vidrob);border-bottom:1px solid var(--vidrob)}
.sb-det dt{color:var(--t3)}
.sb-det dd{margin:0;color:var(--t);min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.sb-det a{color:var(--acct);text-decoration:none}
.sb-det a:hover{text-decoration:underline}
.sb-auto{margin:0;color:var(--t2);font-size:11px;line-height:16px}
"""
cabeca = cabeca.replace("</style>", estilo_extra + "</style>", 1)
cabeca = cabeca.replace("<title>Janela Exportar</title>", "<title>Preferências e Sobre o DataBox</title>")
assert "<title>Preferências e Sobre o DataBox</title>" in cabeca

fechar = re.search(r'<button class="ib" aria-label="Fechar">(<svg.*?</svg>)</button>', base, re.S).group(1)
seta = re.search(r'<button class="campo" aria-label="Separador"[^>]*>[^<]*<span style="flex:1"></span>(<svg.*?</svg>)</button>',
                 base, re.S).group(1)
# as definições do Início têm tudo o que o logo grande usa (lgSombraPeca a mais que as do Exportar)
defs = re.search(r'<svg width="0" height="0"[^>]*><defs>.*?</defs></svg>', inicio, re.S).group(0)
marca = re.search(r'<span class="marca">(<svg.*?</svg>)</span>', inicio, re.S).group(1)
logo = re.search(r'<svg class="logo" viewBox="0 0 512 512" width="60" height="60" aria-hidden="true">.*?</svg>',
                 inicio, re.S).group(0)


def renomear(svg, sufixo):
    """Dá sufixo às definições locais de um SVG (as cópias do logo na página não podem repetir ids)."""
    for i in re.findall(r'id="([^"]+)"', svg):
        svg = svg.replace(f'id="{i}"', f'id="{i}{sufixo}"').replace(f"url(#{i})", f"url(#{i}{sufixo})")
    return svg


def tamanho(svg, lado):
    return re.sub(r'width="\d+" height="\d+"', f'width="{lado}" height="{lado}"', svg, count=1)


ELEMENTO = re.compile(r'<(path|rect|circle|ellipse|polygon|polyline)\b[^>]*>')


def pequeno(svg):
    """Revisão, item 13: num ícone de até 24 px, o corpo (a peça com o icSombra e contorno, ou o contorno de mesmo d
    desenhado depois) vai a 2,4 de largura e .35 de opacidade, e a dobra a 1,6 e .25 (a regra da revisão de 15/10/2026)."""
    els = list(ELEMENTO.finditer(svg))
    sem_traco = {re.search(r' d="([^"]+)"', m.group(0)).group(1) for m in els
                 if 'filter="url(#icSombra)"' in m.group(0) and 'stroke="#000000"' not in m.group(0) and ' d="' in m.group(0)}
    trocas = []
    for m in els:
        tag = m.group(0)
        d = re.search(r' d="([^"]+)"', tag)
        if 'filter="url(#icSombra)"' in tag and 'stroke="#000000"' in tag:
            trocas.append((m, ".35", "2.4"))
        elif 'filter=' not in tag and d and d.group(1) in sem_traco and 'fill="none"' in tag and 'stroke="#000000"' in tag:
            trocas.append((m, ".35", "2.4"))
        elif 'fill="url(#icDobra)"' in tag and 'stroke="#000000"' in tag:
            trocas.append((m, ".25", "1.6"))
    for m, op, lg in sorted(trocas, key=lambda x: x[0].start(), reverse=True):
        tag = m.group(0)
        s = re.search(r'style="([^"]*)"', tag)
        regras = [r for r in (s.group(1).split(";") if s else []) if r and not r.startswith(("stroke-opacity:", "stroke-width:"))]
        estilo = ";".join([f"stroke-opacity:{op}", f"stroke-width:{lg}"] + regras)
        tag = tag[:s.start(1)] + estilo + tag[s.end(1):] if s else tag[:-1] + f' style="{estilo}">'
        svg = svg[:m.start()] + tag + svg[m.end():]
    return svg


def contorno_engrenagem(cx, cy, externo, raiz, dentes, meia_ponta, meia_base, giro=0.0):
    """O contorno de uma engrenagem: dentes retos e, entre um dente e o próximo, um arco na raiz."""
    passo = 360 / dentes
    pontos = []
    for i in range(dentes):
        a = math.radians(i * passo - 90 + giro)
        for raio, d in ((raiz, -meia_base), (externo, -meia_ponta), (externo, meia_ponta), (raiz, meia_base)):
            t = a + math.radians(d)
            pontos.append((cx + raio * math.cos(t), cy + raio * math.sin(t)))
    d = f"M{pontos[0][0]:.2f} {pontos[0][1]:.2f}"
    for k in range(1, len(pontos)):
        x, y = pontos[k]
        d += (f"A{raiz} {raiz} 0 0 1 {x:.2f} {y:.2f}" if k % 4 == 0 else f"L{x:.2f} {y:.2f}")
    x, y = pontos[0]
    return d + f"A{raiz} {raiz} 0 0 1 {x:.2f} {y:.2f}z"


def engrenagem():
    """Ícone sólido de Preferências (16/10/2026): duas engrenagens de metal, a maior na frente (8 dentes) e a menor
    atrás, na diagonal de cima à direita (7 dentes, metal mais escuro), cada uma com o miolo em tampa e o furo escuro."""
    c = 'stroke="#000000" style="stroke-opacity:.35;stroke-linejoin:round"'
    pequena = contorno_engrenagem(34.6, 13.4, 10.4, 7.9, 7, 11, 17, giro=12)
    grande = contorno_engrenagem(19.6, 28.2, 16.6, 12.9, 8, 8.5, 13.5)
    return ('<svg viewBox="0 0 48 48" width="20" height="20"><defs>'
            '<linearGradient id="engrenagens_atras" x1="0" y1="0" x2="1" y2="0"><stop offset="0" style="stop-color:#3f4753"></stop>'
            '<stop offset=".3" style="stop-color:#aab3c0"></stop><stop offset=".6" style="stop-color:#76808f"></stop>'
            '<stop offset="1" style="stop-color:#38404b"></stop></linearGradient></defs>'
            f'<path filter="url(#icSombra)" d="{pequena}" fill="url(#engrenagens_atras)" {c}></path>'
            '<circle cx="34.6" cy="13.4" r="4.6" fill="url(#icTampa)" stroke="#000000" style="stroke-opacity:.25"></circle>'
            '<circle cx="34.6" cy="13.4" r="2" fill="#1d1e21" stroke="#000000" style="stroke-opacity:.4"></circle>'
            f'<path filter="url(#icSombra)" d="{grande}" fill="url(#icMetal)" {c}></path>'
            '<circle cx="19.6" cy="28.2" r="7.6" fill="url(#icTampa)" stroke="#000000" style="stroke-opacity:.25"></circle>'
            '<circle cx="19.6" cy="28.2" r="3.5" fill="#1d1e21" stroke="#000000" style="stroke-opacity:.4"></circle>'
            '<path d="M16.6 27.4a3.2 3.2 0 0 1 6 0" fill="none" stroke="#ffffff" style="stroke-opacity:.2;stroke-width:1.1"></path>'
            '</svg>')


def esquema(nome, nota, fundo, texto, kw, st, nu, pu, on=False):
    p = (f'<span class="cc-p" style="background:{fundo};color:{texto}">'
         f'<span style="color:{kw}">SELECT</span> nome, limite\n'
         f'  <span style="color:{kw}">FROM</span> <span style="color:{st}">"clientes"</span>\n'
         f' <span style="color:{kw}">WHERE</span> limite <span style="color:{pu}">&gt;</span> <span style="color:{nu}">1000</span>'
         f'<span style="color:{pu}">;</span></span>')
    classe = "cc-c on" if on else "cc-c"
    return (f'<button class="{classe}" role="radio" aria-checked="{"true" if on else "false"}">{p}'
            f'<span class="cc-n">{nome}<small>{nota}</small></span></button>')


def linha(rotulo, dica, controle, alto=False):
    classe = "pref-l alto" if alto else "pref-l"
    return (f'<div class="{classe}"><div class="pref-r"><b>{rotulo}</b><span>{dica}</span></div>'
            f'<div style="min-width:0">{controle}</div></div>')


def campo_lista(texto, largura=None):
    estilo = f' style="width:{largura}px;flex:none"' if largura else ' style="flex:1"'
    return f'<button class="campo"{estilo}>{texto}<span style="flex:1"></span>{seta}</button>'


# ícones das seções (16/10/2026): controles deslizantes (Geral), paleta de pintor (Aparência), a tabela da família
# (Tabelas), o documento SQL (Consulta SQL) e o documento com a seta de exportar (Exportação)
C35 = 'stroke="#000000" style="stroke-opacity:.35;stroke-linejoin:round"'
ICONE_GERAL = (
    '<svg viewBox="0 0 48 48" width="20" height="20"><defs><linearGradient id="pref_geral_placa" x1="0" y1="0" x2="0" y2="1">'
    '<stop offset="0" style="stop-color:#66717f"></stop><stop offset="1" style="stop-color:#333a44"></stop></linearGradient></defs>'
    f'<rect filter="url(#icSombra)" x="5" y="7" width="38" height="34" rx="7" fill="url(#pref_geral_placa)" {C35}></rect>'
    '<rect x="5.5" y="7.5" width="37" height="14" rx="6.5" fill="url(#icRealce)" style="opacity:.35"></rect>'
    + "".join(f'<rect x="11" y="{y - 1.6}" width="26" height="3.2" rx="1.6" fill="#191c21"></rect>'
              f'<rect x="11" y="{y - 1.6}" width="{k - 11}" height="3.2" rx="1.6" fill="#4f8ff0"></rect>'
              f'<circle cx="{k}" cy="{y}" r="4.3" fill="url(#icTampa)" {C35}></circle>'
              f'<circle cx="{k - 1}" cy="{y - 1.2}" r="1.4" fill="#ffffff" style="fill-opacity:.6"></circle>'
              for y, k in ((15.5, 30), (24, 17.5), (32.5, 25.5)))
    + "</svg>")
ICONE_APARENCIA = (
    '<svg viewBox="0 0 48 48" width="20" height="20"><defs><linearGradient id="pref_paleta_madeira" x1="0" y1="0" x2="1" y2="1">'
    '<stop offset="0" style="stop-color:#efd29a"></stop><stop offset="1" style="stop-color:#b5813f"></stop></linearGradient></defs>'
    '<path filter="url(#icSombra)" d="M24 6c11 0 20 7 20 16 0 6-5 8.2-10 7.2-3.8-.7-5.8 1.9-4.4 5.2C31.4 39 28.4 42 23 42 12 42 4 34.5 4 24 '
    '4 13.5 13 6 24 6zM19.6 29a3.4 3.4 0 1 0 0 6.8 3.4 3.4 0 0 0 0-6.8z" fill="url(#pref_paleta_madeira)" '
    'stroke="#000000" style="stroke-opacity:.35;stroke-linejoin:round;fill-rule:evenodd"></path>'
    '<path d="M9 18c3-6.5 9-9.6 15-9.6" fill="none" stroke="#ffffff" style="stroke-opacity:.5;stroke-width:1.3;stroke-linecap:round"></path>'
    + "".join(f'<circle cx="{x}" cy="{y}" r="3.9" fill="{c}" stroke="#000000" style="stroke-opacity:.3"></circle>'
              f'<circle cx="{x - 1.1}" cy="{y - 1.2}" r="1.2" fill="#ffffff" style="fill-opacity:.55"></circle>'
              for x, y, c in ((15.5, 16, "#e0403a"), (25, 12.6, "#f3c331"), (34.3, 16.2, "#2c6fdc"), (11.4, 25, "#2e9e5b")))
    + "</svg>")
ICONE_EXPORTAR = (
    '<svg viewBox="0 0 48 48" width="20" height="20"><path filter="url(#icSombra)" d="M7 4h17l11 11v27a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z" '
    'fill="url(#icPapel)" stroke="#000000" style="stroke-opacity:.3"></path><path d="M24 4v9a2 2 0 0 0 2 2h9" fill="url(#icDobra)" '
    'stroke="#000000" style="stroke-opacity:.25"></path><path d="M10 12.5h9M10 17h13" stroke="#000000" '
    'style="stroke-opacity:.13;stroke-width:1.8;stroke-linecap:round"></path><path filter="url(#icSombra)" '
    'd="M18 26h14.5v-6.5L44.5 30l-12 10.5V34H18a2 2 0 0 1-2-2v-4a2 2 0 0 1 2-2z" fill="#2c6fdc"></path>'
    '<path d="M16 30v-2a2 2 0 0 1 2-2h14.5v-6.5L44.5 30z" fill="url(#icRealce)"></path>'
    '<path d="M16 30v2a2 2 0 0 0 2 2h14.5v6.5L44.5 30z" fill="url(#icFundo)"></path>'
    '<path d="M18 26h14.5v-6.5L44.5 30l-12 10.5V34H18a2 2 0 0 1-2-2v-4a2 2 0 0 1 2-2z" fill="none" stroke="#000000" '
    'style="stroke-opacity:.35;stroke-linejoin:round"></path><path d="M18.6 27.4h12.7" stroke="#ffffff" '
    'style="stroke-opacity:.5;stroke-width:1.2;stroke-linecap:round"></path></svg>')
icone_tabela = re.search(r'<label class="op"><input type="checkbox" checked=""><span class="sk s16" aria-hidden="true">'
                         r'(<svg viewBox="0 0 48 48" width="16" height="16">(?:(?!</svg>).)*</svg>)</span>clientes</label>',
                         base, re.S).group(1)
# o documento CSV do seletor Formato (3 letras, sem a classe l4 desde a revisão), com a etiqueta pintada de SQL
doc_csv = re.search(r'<span class="sk s16 arq" aria-hidden="true">(<svg viewBox="0 0 48 48"(?:(?!</svg>).)*</svg>)'
                    r'<span class="rot">CSV</span>', base, re.S).group(1)
ICONE_SQL, n = re.subn(r'(<rect x="2" y="21" width="40" height="21" rx="3.5" fill=")#[0-9a-f]{6}"', r'\1#2c6fdc"', doc_csv)
assert n == 1, "a etiqueta do documento CSV no Exportar"
ICONE_SQL = tamanho(ICONE_SQL, 20)
ICONES_SECOES = {
    "Geral": f'<span class="sk" aria-hidden="true">{pequeno(ICONE_GERAL)}</span>',
    "Aparência": f'<span class="sk" aria-hidden="true">{pequeno(ICONE_APARENCIA)}</span>',
    "Tabelas": f'<span class="sk" aria-hidden="true">{tamanho(icone_tabela, 20)}</span>',
    "Consulta SQL": f'<span class="sk arq" aria-hidden="true">{ICONE_SQL}<span class="rot">SQL</span></span>',
    "Exportação": f'<span class="sk" aria-hidden="true">{pequeno(ICONE_EXPORTAR)}</span>',
}
secoes = ["Geral", "Aparência", "Tabelas", "Consulta SQL", "Exportação"]
nav = "".join(f'<a class="no on" href="#" aria-current="page">{ICONES_SECOES[s]}{s}</a>' if s == "Aparência"
              else f'<a class="no" href="#">{ICONES_SECOES[s]}{s}</a>' for s in secoes)

aparencia = (
    '<h3>Aparência</h3>'
    + linha("Tema", "Cores da janela, dos painéis e dos documentos.",
            '<div class="seg" role="group" aria-label="Tema"><button class="on" aria-pressed="true">Seguir o Windows</button>'
            '<button aria-pressed="false">Claro</button><button aria-pressed="false">Escuro</button></div>')
    + linha("Cores do código", "Consulta SQL, JSON, texto e o SQL que as janelas mostram.",
            '<div class="cc" role="radiogroup" aria-label="Cores do código">'
            + esquema("DataBox", "segue o tema", "#191a1d", "#ececee", "#8ab4f8", "#7fd1a8", "#f2b65e", "#9597a0", on=True)
            + esquema("Alto contraste", "", "#000000", "#ffffff", "#66c2ff", "#8cff8c", "#ffd866", "#c8c8c8")
            + esquema("Solarized", "escuro", "#002b36", "#93a1a1", "#859900", "#2aa198", "#d33682", "#657b83")
            + esquema("Monokai", "", "#272822", "#f8f8f2", "#f92672", "#e6db74", "#ae81ff", "#a59f85")
            + "</div>", alto=True)
    + linha("Fonte do código", "Também vale para os números das linhas.",
            '<div class="pref-fonte">' + campo_lista("Consolas") + campo_lista("12,5 px", 92) + "</div>")
)

# D3: cada mudança vale na hora; Salvar só liga depois de alguma mudança (aqui nada mudou ainda)
VOLTA = "Volta ao que estava quando a janela abriu"
pref = ('<section class="dlg flut pref" role="dialog" aria-label="Preferências">'
        f'<header><span class="sk s20" aria-hidden="true">{pequeno(engrenagem())}</span><h2 style="margin-left:10px">Preferências</h2>'
        f'<span style="flex:1"></span><button class="ib" aria-label="Fechar" title="Fechar (Esc): {VOLTA.lower()}">{fechar}</button></header>'
        f'<div class="pref-corpo"><nav class="pref-nav" aria-label="Seções">{nav}</nav><div class="pref-sec">{aparencia}</div></div>'
        '<footer><button class="btn sutil" title="Restaura o padrão só desta seção (Aparência)">Restaurar o padrão</button>'
        '<span style="flex:1"></span>'
        f'<button class="btn sutil" title="{VOLTA} (Esc)">Cancelar</button>'
        '<button class="btn pri" disabled="" title="As mudanças já valem na hora; Salvar guarda para as próximas aberturas">Salvar</button>'
        '</footer></section>')

sobre = ('<section class="dlg flut sobre" role="dialog" aria-label="Sobre o DataBox">'
         f'<header><span class="sk s20" aria-hidden="true">{tamanho(renomear(marca, "Sb"), 20)}</span>'
         '<h2 style="margin-left:10px">Sobre o DataBox</h2><span style="flex:1"></span>'
         f'<button class="ib" aria-label="Fechar">{fechar}</button></header>'
         '<div class="sb-corpo">'
         f'<div class="sb-topo">{tamanho(renomear(logo, "Sg"), 96)}<div class="sb-id"><span class="sb-nome">DataBox</span>'
         '<span class="sb-ver">Versão 0.1.0</span>'
         '<span class="sb-desc">Toolkit de dados gratuito e aberto para desenvolvedores, cientistas e engenheiros de dados.</span>'
         '</div></div>'
         '<dl class="sb-det">'
         '<dt>Compilação</dt><dd><span class="mono">15/10/2026 · fb34ec7</span></dd>'
         '<dt>Renderizador</dt><dd>OpenGL (glow)</dd>'
         '<dt>SQLite</dt><dd>3.53.2, embutido</dd>'
         '<dt>Sistema</dt><dd>Windows 11 · x64</dd>'
         '<dt>Licença</dt><dd><a href="#" title="Abrir o texto da licença">MIT</a></dd>'
         '<dt>Preferências</dt><dd><a href="#" class="mono" title="Abrir a pasta no Explorador">%APPDATA%\\DataBox</a></dd>'
         '<dt>Arquivo de log</dt><dd><a href="#" class="mono" title="Abrir o arquivo">%LOCALAPPDATA%\\DataBox\\databox.log</a></dd>'
         '</dl>'
         '<p class="sb-auto">Desenvolvido por Isaías Gonçalves. Feito em Rust, com egui e SQLite.</p>'
         '</div>'
         '<footer><button class="btn sutil">Licenças de terceiros</button><span style="flex:1"></span>'
         '<button class="btn">Copiar informações</button><button class="btn pri">Fechar</button></footer></section>')

inicio_fundo = base.index('<div aria-hidden="true" style="flex:1;display:flex;flex-direction:column">')
fim_fundo = base.index('<div style="position:absolute;inset:0;background:var(--veu)')
fundo_html = base[inicio_fundo:fim_fundo]

corpo = ('<div class="est {{classes}}" style="width:100%;min-height:900px;height:900px;display:flex;flex-direction:column;'
         'overflow:hidden;position:relative">\n' + defs + "\n" + fundo_html
         + '<div style="position:absolute;inset:0;background:var(--veu);display:flex;align-items:center;justify-content:center;'
         'gap:40px;padding:24px">\n' + pref + "\n" + sobre + "\n</div>\n</div>")

script = base[base.index('<script type="text/x-dc"'):]
pagina = cabeca + "</helmet>\n" + corpo + "\n</x-dc>\n" + script
(PASTA / "Preferencias.dc.html").write_text(pagina, encoding="utf-8", newline="\n")
print("Preferencias.dc.html", len(pagina), "bytes")
