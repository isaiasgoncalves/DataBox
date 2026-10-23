#!/usr/bin/env python3
"""As pranchetas da página "Propostas 0.3" do canvas (revisão de UI de 22/10/2026, `design/revisao-ui-0.3.md`).

Cada proposta sai de uma prancheta original (Dados, CSV, JSON, Consulta, Visão geral, perfil do roadmap e
Preferências), com a folha de estilo, os ícones e as peças dela, e muda só o que a proposta muda. As etiquetas âmbar
(U1, U2…) apontam os achados. A `Propostas03Hoje` (as capturas de antes) não sai daqui: ela usa as imagens já
enviadas ao canvas e fica como está.

Refaz as pranchetas inteiras e recusa a original que mudou a ponto de um trecho esperado não estar mais nela. Não
mexe no `canvas.json`. Rode de qualquer pasta: lê e grava em `../canvas`.
"""
import os
import re
import sys

CANVAS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "canvas")

# a etiqueta dos achados, igual em todas
CSS_MARCA = (
    ".u-marca{position:absolute;z-index:80;display:inline-flex;align-items:center;justify-content:center;"
    "height:20px;min-width:30px;padding:0 7px;border-radius:999px;background:#f2b65e;color:#1b1c1f;"
    "font:800 11px/1 'Segoe UI Variable Text','Segoe UI',system-ui,sans-serif;"
    "box-shadow:0 0 0 2px rgba(0,0,0,.55),0 2px 6px rgba(0,0,0,.5);pointer-events:none}\n"
)


def ler(nome):
    with open(os.path.join(CANVAS, nome), encoding="utf-8") as f:
        return f.read()


def trocar(s, antes, depois, n=1):
    if s.count(antes) < n:
        sys.exit(f"trecho esperado não encontrado: {antes[:80]!r}")
    return s.replace(antes, depois, n)


def trocar_re(s, padrao, depois, flags=re.S):
    novo, n = re.subn(padrao, lambda _m: depois, s, count=1, flags=flags)
    if n != 1:
        sys.exit(f"padrão esperado não encontrado: {padrao[:80]!r}")
    return novo


def bloco_div(s, inicio):
    """O `<div …>…</div>` que começa em `inicio`, com os `div` de dentro (fim exclusivo)."""
    nivel, i = 0, inicio
    for m in re.finditer(r"<div\b|</div>", s[inicio:]):
        nivel += 1 if m.group() == "<div" else -1
        if nivel == 0:
            return inicio, inicio + m.end()
    sys.exit("div sem fim")


def tirar_div(s, marca):
    i = s.find(marca)
    if i < 0:
        sys.exit(f"div esperado não encontrado: {marca[:80]!r}")
    a, b = bloco_div(s, i)
    return s[:a] + s[b:]


def svg_do_botao(s, rotulo):
    """O `<svg>` do primeiro botão de ícone com este `aria-label` (a silhueta da barra)."""
    m = re.search(rf'aria-label="{re.escape(rotulo)}"[^>]*>(<svg\b.*?</svg>)', s, re.S)
    if not m:
        sys.exit(f"botão sem silhueta: {rotulo}")
    return m.group(1)


def icone_arquivo(s, ext):
    """A folha com a etiqueta da extensão (`.sk.s16.arq`), como a aba a desenha."""
    i = s.find(f'<span class="rot">{ext}</span></span>')
    if i < 0:
        sys.exit(f"ícone de arquivo não encontrado: {ext}")
    j = s.rfind('<span class="sk s16 arq', 0, i)
    return s[j : i + len(f'<span class="rot">{ext}</span></span>')]


def pastilha(s, glifo):
    """A pastilha de tipo (`.sk.s16.tipo`) com este glifo, como o cabeçalho da grade a desenha."""
    fim = f'<span class="rot">{glifo}</span></span>'
    i = s.find(fim)
    if i < 0:
        sys.exit(f"pastilha não encontrada: {glifo}")
    j = s.rfind('<span class="sk s16 tipo', 0, i)
    return s[j : i + len(fim)]


def acabamento(s, titulo, marcas):
    """O título da prancheta, o CSS das etiquetas e as etiquetas (achado, x, y) no canto de cima à esquerda."""
    s = trocar_re(s, r"<title>.*?</title>", f"<title>{titulo}</title>")
    s = trocar(s, "</style>\n</helmet>", CSS_MARCA + "</style>\n</helmet>")
    html = "".join(f'<span class="u-marca" style="left:{x}px;top:{y}px">{u}</span>' for u, x, y in marcas)
    i = s.rindex("</x-dc>")
    j = s.rindex("</div>", 0, i)
    return s[:j] + html + "\n" + s[j:]


def numero_br(t):
    """'14542.56' → '14.542,56', sem arredondar (as casas gravadas, todas)."""
    inteira, _, fracao = t.partition(".")
    grupos = []
    while len(inteira) > 3:
        grupos.insert(0, inteira[-3:])
        inteira = inteira[:-3]
    grupos.insert(0, inteira)
    return ".".join(grupos) + ("," + fracao if fracao else "")


# ------------------------------------------------------------------------------------------- U1 U2 U3: a grade


def grade():
    s = ler("Dados.dc.html")
    # U1: os REAL no formato do idioma, sem arredondar
    for valor in re.findall(r"'(\d+(?:\.\d+)?)', '2026-", s):
        s = trocar(s, f"'{valor}', '2026-", f"'{numero_br(valor)}', '2026-")
    # U3: a contagem sai da faixa
    s = trocar(s, '<span style="color:var(--t3)">41 de 60 linhas</span>\n', "")
    # U2 e U3: o rodapé rola, sem páginas, e diz de quantas a busca e os filtros tiraram
    s = trocar_re(
        s,
        r'<div class="rodape">.*?</div>',
        '<div class="rodape">\n<span>Linhas 1–18 de 41</span><span style="color:var(--t3)">· filtradas de 60 · 2 '
        'marcadas</span>\n<span style="flex:1"></span>\n</div>',
    )
    s = tirar_div(s, '<div role="menu" aria-label="Tabela"')
    s = trocar(s, '<button class="menu on">Tabela</button>', '<button class="menu">Tabela</button>')
    return acabamento(
        s,
        "Uma grade só",
        [("U1", 880, 236), ("U3", 1000, 132), ("U2", 1000, 830), ("U3", 300, 830)],
    )


# ------------------------------------------------------------------------------------------- U5: os vazios


def vazio_dos_dados():
    s = ler("Dados.dc.html")
    s = trocar(s, "linhas: dados.map((d) => ({", "linhas: dados.slice(0, 0).map((d) => ({")
    s = trocar(s, "<b>ativo</b> igual a <b>1</b>", "<b>cidade</b> igual a <b>Brasília</b>")
    s = trocar(s, '<span style="color:var(--t3)">41 de 60 linhas</span>\n', "")
    s = trocar(
        s,
        "</tbody>\n</table>\n</div>\n<div class=\"rodape\">",
        "</tbody>\n</table>\n"
        '<div style="display:flex;flex-direction:column;align-items:center;gap:12px;padding-top:200px">'
        '<strong style="font-size:13px;color:var(--t)">Nenhuma linha corresponde à busca ou aos filtros.</strong>'
        '<button class="btn">Limpar filtros</button></div>\n'
        '</div>\n<div class="rodape">',
    )
    s = trocar_re(
        s,
        r'<div class="rodape">.*?</div>',
        '<div class="rodape">\n<span>Nenhuma linha</span><span style="color:var(--t3)">· filtradas de 60</span>\n</div>',
    )
    s = tirar_div(s, '<div role="menu" aria-label="Tabela"')
    s = trocar(s, '<button class="menu on">Tabela</button>', '<button class="menu">Tabela</button>')
    return acabamento(s, "Vazio com filtro", [("U5", 520, 372)])


def vazio_do_csv():
    s = ler("Csv.dc.html")
    nova = svg_do_botao(s, "Nova linha")
    # a faixa saiu na 0.2.3: o formato foi para o rodapé
    s = trocar_re(s, r'<div class="faixa"><strong style="font-size:13px">vendas_2025\.csv</strong>.*?</div>\n', "")
    s = trocar_re(s, r"linhas: linhas\b", "linhas: []")
    s = trocar(
        s,
        "</tbody>\n</table>\n</div>\n<div class=\"rodape\">",
        "</tbody>\n</table>\n"
        '<div style="display:flex;flex-direction:column;align-items:center;gap:12px;padding-top:200px">'
        '<strong style="font-size:13px;color:var(--t)">Nenhuma linha neste arquivo.</strong>'
        f'<button class="btn pri">{nova}Nova linha</button></div>\n'
        '</div>\n<div class="rodape">',
    )
    s = trocar_re(
        s,
        r'<div class="rodape">.*?</div>',
        '<div class="rodape"><span>Nenhuma linha</span><span style="color:var(--t3)">· Ponto e vírgula · UTF-8 · '
        "CRLF</span></div>",
    )
    return acabamento(s, "Vazio do CSV", [("U5", 540, 330)])


# ------------------------------------------------------------------------------------------- U4 e U6: faixas


def json():
    s = ler("Json.dc.html")
    # U4: a migalha começa na raiz; o nome do arquivo já está na aba
    s = trocar_re(
        s,
        r'<span class="migalha"><span class="sk s16 arq l4" aria-hidden="true">.*?<span class="rot">JSON</span></span>'
        r'<span style="margin-left:3px">config\.json</span>',
        '<span class="migalha"><span>raiz</span>',
    )
    # U6: o caminho comprido perde o começo, não o meio
    s = trocar(
        s,
        '<dd class="mono caminho" title="C:\\dados\\config.json"><span class="ini">C:\\dados\\</span>'
        '<span class="fim">config.json</span></dd>',
        '<dd class="mono caminho" title="C:\\Users\\ana\\Documentos\\projetos\\painel\\config.json">'
        '<span class="fim">…\\painel\\config.json</span></dd>',
    )
    return acabamento(s, "JSON com a migalha", [("U4", 300, 132), ("U6", 1290, 500)])


def consulta():
    s = ler("Consulta.dc.html")
    csv = icone_arquivo(ler("Dados.dc.html"), "CSV")
    mais = svg_do_botao(ler("Dados.dc.html"), "Nova linha")
    # U4 e P4: a faixa das fontes (o banco e os arquivos da bancada); a posição do cursor desce para o rodapé
    m = re.search(
        r'<div class="faixa">(<span class="sk s16" aria-hidden="true">.*?</span>)<span style="color:var\(--t2\)">'
        r'exemplo\.db</span><span style="flex:1"></span><span style="color:var\(--t3\)">Linha 4, coluna 1</span></div>',
        s,
        re.S,
    )
    if not m:
        sys.exit("a faixa da Consulta mudou")
    banco = m.group(1)
    faixa = (
        '<div class="faixa"><span class="tit">Fontes</span>'
        f'<span class="chip">{banco}<b>exemplo.db</b></span>'
        f'<span class="chip">{csv}<b>vendas_2025.csv</b>&nbsp;como <b>arquivos.vendas_2025</b></span>'
        f'<button class="btn sutil pq">{mais}Arquivo</button></div>'
    )
    s = s[: m.start()] + faixa + s[m.end() :]
    s = trocar(
        s,
        '<div class="rodape"><span>8 linhas · 0,5 ms</span></div>',
        '<div class="rodape"><span>8 linhas · 0,5 ms</span><span style="flex:1"></span>'
        '<span style="color:var(--t3)">Linha 4, coluna 1</span></div>',
    )
    return acabamento(s, "Consulta com as fontes", [("U4", 300, 132), ("U4", 1060, 830)])


def visao_geral():
    s = ler("Main.dc.html")
    colunas = svg_do_botao(ler("Dados.dc.html"), "Nova coluna")
    # U4: sem faixa (o nome na aba, o caminho nas Propriedades e na dica da aba)
    s = trocar_re(s, r'<div class="faixa"><span class="sk s20" aria-hidden="true">.*?C:\\dados\\exemplo\.db</span></div>\n', "")
    # U8: a etiqueta do tipo ao lado do nome; o "Colunas" vira botão de ícone; a coluna Tipo sai
    s = trocar(
        s,
        '<th style="width:150px">Tipo</th><th style="width:130px"></th>',
        '<th style="width:56px"></th>',
    )
    s = trocar(
        s,
        "{{t.nome}}</a></td>",
        '{{t.nome}}</a><sc-if value="{{t.temTipo}}" hint-placeholder-val="{{ false }}">'
        '<span class="pill" style="margin-left:8px">{{t.tipo}}</span></sc-if></td>',
    )
    s = trocar_re(
        s,
        r'<td><sc-if value="\{\{t\.temTipo\}\}" hint-placeholder-val="\{\{ false \}\}"><span class="pill">\{\{t\.tipo\}\}</span></sc-if></td>\n'
        r'<td style="text-align:right"><button class="btn sutil pq">Colunas</button></td>',
        f'<td style="text-align:right"><button class="ib pq" aria-label="Colunas de {{{{t.nome}}}}" '
        f'title="Colunas de {{{{t.nome}}}}">{colunas}</button></td>',
    )
    return acabamento(s, "Visão geral sem faixa", [("U4", 300, 132), ("U8", 760, 420)])


# ------------------------------------------------------------------------------------------- P1: o perfil


# a linha do perfil escolhida (a coluna que as Propriedades mostram)
SELECIONADA = ' style="background:var(--selh)"'


def perfil():
    s = ler("FuturoPerfil.dc.html")
    p = {g: pastilha(s, g) for g in ("PK", "Aa", ",5", "12")}
    atualizar = svg_do_botao(s, "Atualizar")
    exportar = svg_do_botao(s, "Exportar")
    linhas = [
        ("PK", "id", "60", "0", "60", "1", "60", "30,5", "—", [5, 5, 5, 5, 5, 5, 5, 5, 5, 5]),
        ("Aa", "nome", "60", "0", "54", "Ana Coutinho", "Tiago Esteves", "—", "Lucas Brandão (3)", None),
        ("Aa", "email", "60", "0", "60", "ana.12@…", "tiago.13@…", "—", "todos distintos", None),
        ("Aa", "cidade", "60", "0", "8", "Belo Horizonte", "São Paulo", "—", "São Paulo (14)", None),
        (",5", "limite", "57", "3", "57", "154,03", "19.437,39", "9.812,40", "—", [3, 7, 12, 18, 22, 15, 9, 6, 4, 2]),
        ("12", "ativo", "60", "0", "2", "0", "1", "0,92", "1 (55)", [5, 0, 0, 0, 0, 0, 0, 0, 0, 55]),
        ("Aa", "criado_em", "60", "0", "60", "2026-01-05", "2026-07-01", "—", "—", None),
    ]

    def hist(v):
        if not v:
            return '<span style="color:var(--t3)">—</span>'
        m = max(v)
        barras = "".join(
            f'<span style="flex:1;height:{max(1, round(18 * x / m))}px;border-radius:2px 2px 0 0;background:var(--acc);'
            'opacity:.85"></span>'
            for x in v
        )
        return f'<span style="display:flex;gap:2px;align-items:flex-end;height:18px;width:90px">{barras}</span>'

    corpo = "".join(
        f'<tr{SELECIONADA if n == "limite" else ""}>'
        f'<td>{p[g]}<b>{n}</b></td><td class="num">{pre}</td>'
        f'<td class="num">{nul}</td><td class="num">{dis}</td>'
        f'<td class="num">{mn}</td><td class="num">{mx}</td><td class="num">{med}</td><td>{freq}</td><td>{hist(h)}</td></tr>'
        for g, n, pre, nul, dis, mn, mx, med, freq, h in linhas
    )
    doc = (
        '<div class="doc" style="flex:1">\n'
        '<div class="faixa"><span style="color:var(--t2)">clientes · 60 linhas · 7 colunas</span>'
        '<span style="flex:1"></span><span style="color:var(--t3)">Exato até 100 mil valores; acima, ≈</span>'
        f'<button class="btn sutil pq">{atualizar}Recalcular</button>'
        f'<button class="btn sutil pq">{exportar}Exportar o perfil</button></div>\n'
        '<div style="flex:1;min-height:0;overflow:hidden">\n<table class="grade">\n<thead><tr>'
        '<th>Coluna</th><th class="num">Preenchidas</th><th class="num">Nulos</th><th class="num">Distintos</th>'
        '<th class="num">Mínimo</th><th class="num">Máximo</th><th class="num">Média</th><th>Mais frequente</th>'
        f"<th>Distribuição</th></tr></thead>\n<tbody>{corpo}</tbody>\n</table>\n</div>\n"
        '<div class="rodape"><span>7 colunas</span><span style="color:var(--t3)">· calculado em 0,2 s</span></div>\n'
        "</div>\n"
    )
    i = s.index('<div class="doc"')
    a, b = bloco_div(s, i)
    s = s[:a] + doc + s[b:]
    # a aba nova do perfil, ativa, depois da aba da tabela
    m = re.search(r'<div class="aba on" role="presentation"><a href="#" role="tab" aria-selected="true">(.*?)clientes</a>', s, re.S)
    if not m:
        sys.exit("a aba de clientes mudou")
    icone = m.group(1)
    fechar = re.search(r'<button class="x" aria-label="Fechar aba">.*?</button>', s, re.S)
    if not fechar:
        sys.exit("a aba sem o ×")
    fechar = fechar.group(0)
    s = s[: m.start()] + s[m.start() :].replace(
        '<div class="aba on" role="presentation"><a href="#" role="tab" aria-selected="true">',
        '<div class="aba" role="presentation"><a href="#" role="tab" aria-selected="false">',
        1,
    )
    fim_aba = s.index("</div>", m.start()) + len("</div>")
    nova = (
        f'<div class="aba on" role="presentation"><a href="#" role="tab" aria-selected="true">{icone}Perfil'
        f'<span class="sub">clientes</span></a>{fechar}</div>'
    )
    s = s[:fim_aba] + nova + s[fim_aba:]
    return acabamento(s, "Perfil da tabela", [("P1", 300, 132)])


# ------------------------------------------------------------------------------------------- P7 e U1: preferências


def preferencias():
    s = ler("Preferencias.dc.html")
    seta = re.search(r'<button class="campo"[^>]*>.*?(<svg\b.*?</svg>)</button>', ler("Csv.dc.html"), re.S).group(1)
    tema_fim = s.index('<div class="pref-l alto"><div class="pref-r"><b>Cores do código</b>')
    novas = (
        '<div class="pref-l"><div class="pref-r"><b>Idioma</b><span>Menus, janelas e mensagens; os números e as '
        'datas seguem o idioma.</span></div><div style="min-width:0"><button class="campo" aria-label="Idioma" '
        f'style="width:100%">Seguir o sistema (Português)<span style="flex:1"></span>{seta}</button></div></div>'
        '<div class="pref-l"><div class="pref-r"><b>Números na grade</b><span>Nos Dados e na Consulta. O Copiar '
        'leva sempre o valor gravado.</span></div><div style="min-width:0"><div class="seg" role="group" '
        'aria-label="Números na grade"><button class="on" aria-pressed="true">Formatados</button>'
        '<button aria-pressed="false">Como gravados</button></div></div></div>'
    )
    s = s[:tema_fim] + novas + s[tema_fim:]
    return acabamento(s, "Preferências com o idioma", [("P7", 470, 300), ("U1", 470, 360)])


PRANCHETAS = {
    "Propostas03Grade.dc.html": grade,
    "Propostas03Vazia.dc.html": vazio_dos_dados,
    "Propostas03VaziaCsv.dc.html": vazio_do_csv,
    "Propostas03Faixas.dc.html": json,
    "Propostas03Consulta.dc.html": consulta,
    "Propostas03Visao.dc.html": visao_geral,
    "Propostas03Perfil.dc.html": perfil,
    "Propostas03Preferencias.dc.html": preferencias,
}

if __name__ == "__main__":
    destino = sys.argv[1] if len(sys.argv) > 1 else CANVAS
    for nome, f in PRANCHETAS.items():
        with open(os.path.join(destino, nome), "w", encoding="utf-8") as saida:
            saida.write(f())
        print(nome)
