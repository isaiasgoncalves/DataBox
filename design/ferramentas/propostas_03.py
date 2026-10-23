#!/usr/bin/env python3
"""Gera as pranchetas da página "Propostas 0.3" do canvas (revisão de UI de 22/10/2026, `design/revisao-ui-0.3.md`).

Refaz as seis pranchetas `Propostas03*.dc.html` inteiras. A `Propostas03Hoje` usa as capturas já enviadas ao
canvas (`/_blob/...`). Não mexe no `canvas.json`.
"""
import os

RAIZ = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "canvas")

CSS = """
body{margin:0;background:#0e0f11}
.est{--acc:#2c6fdc;--fundo0:#111214;--painel:#1e1f22;--c:#191a1d;--p:#212226;--t:#ececee;--t2:#b9bbc0;--t3:#9597a0;--bt1:#3b3c40;--bt2:#26272a;--btb:rgba(0,0,0,.6);--btluz:rgba(255,255,255,.13);--fb:rgba(255,255,255,.14);--fbc:rgba(255,255,255,.40);--b:rgba(255,255,255,.07);--perigo:#ff8a80;--ok:#5fd09a;--aviso:#f2b65e;--kw:#8ab4f8;--nu:#f2b65e;--st:#7fd1a8;--chave:#c9a0ff;--sel:rgba(44,111,220,.28);font-family:"Segoe UI Variable Text","Segoe UI","Noto Sans",system-ui,sans-serif;font-size:12px;color:var(--t);background:var(--fundo0);font-variant-numeric:tabular-nums}
.est *{box-sizing:border-box}
.janela{display:flex;flex-direction:column;overflow:hidden;background:var(--fundo0)}
.titulo{height:30px;flex:none;display:flex;align-items:center;gap:2px;padding-left:10px;color:var(--t2)}
.titulo .menu{padding:4px 10px;border-radius:999px}
.titulo .nome{flex:1;text-align:center;color:var(--t3)}
.titulo .jb{width:46px;text-align:center;color:var(--t2)}
.logo{width:16px;height:16px;border-radius:4px;background:linear-gradient(#d9a95a,#8c5a22);margin-right:8px;flex:none}
.barra{height:44px;flex:none;margin:6px 8px 8px;border-radius:10px;background:var(--painel);border:1px solid var(--b);box-shadow:inset 0 1px 0 rgba(255,255,255,.05);display:flex;align-items:center;gap:2px;padding:0 8px}
.ib{width:30px;height:30px;border-radius:50%;border:1px solid transparent;background:transparent;color:var(--t2);display:inline-flex;align-items:center;justify-content:center;padding:0;font:inherit}
.ib.off{opacity:.3}
.sep{width:1px;height:18px;background:var(--fb);margin:0 6px}
.busca{width:250px;height:28px;border-radius:10px;border:1px solid var(--fbc);background:#141518;display:flex;align-items:center;gap:6px;padding:0 6px 0 10px;color:var(--t3);margin:0 6px}
.kbd{border:1px solid var(--fb);border-radius:5px;padding:0 5px;font-size:10.5px;color:var(--t3);line-height:16px}
.corpo{flex:1;min-height:0;display:flex;gap:8px;padding:0 8px 8px}
.coluna{display:flex;flex-direction:column;min-width:0;min-height:0}
.doca{background:var(--painel);border:1px solid var(--b);border-radius:10px;display:flex;flex-direction:column;overflow:hidden;min-height:0}
.abas{height:34px;flex:none;display:flex;align-items:flex-end;gap:4px;padding:0 6px}
.aba{height:30px;display:flex;align-items:center;gap:7px;padding:0 12px;border-radius:10px 10px 0 0;color:var(--t2);white-space:nowrap}
.aba.on{background:linear-gradient(var(--bt1),var(--bt2));color:var(--t);box-shadow:inset 0 1px 0 var(--btluz)}
.aba .x{color:var(--t3);margin-left:4px}
.aba .sub{color:var(--t3)}
.doc{flex:1;min-height:0;background:var(--c);border:1px solid var(--b);border-radius:10px;overflow:hidden;display:flex;flex-direction:column}
.faixa{height:42px;flex:none;display:flex;align-items:center;gap:8px;padding:0 10px 0 14px;background:var(--p);border-bottom:1px solid var(--b)}
.rot{font-size:11px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;color:var(--t3)}
.pilula{display:inline-flex;align-items:center;gap:5px;height:24px;padding:0 10px;border-radius:999px;border:1px solid rgba(44,111,220,.6);background:rgba(44,111,220,.16);color:var(--t)}
.pilula.neutra{border-color:var(--fb);background:rgba(255,255,255,.04)}
.lnk{color:var(--t);font-weight:600}
.rodape{height:34px;flex:none;display:flex;align-items:center;gap:8px;padding:0 14px;background:var(--p);border-top:1px solid var(--b);color:var(--t2);white-space:nowrap}
.rodape .mais{margin-left:auto}
.ponto{color:var(--t3)}
.grade{flex:1;min-height:0;overflow:hidden;position:relative}
.grade table{border-collapse:collapse;width:100%;table-layout:fixed}
.grade th{height:30px;text-align:left;font-weight:600;color:var(--t);background:linear-gradient(var(--bt1),var(--bt2));border-right:1px solid var(--b);border-bottom:1px solid var(--btb);padding:0 8px;white-space:nowrap;overflow:hidden}
.grade td{height:28px;padding:0 8px;border-right:1px solid var(--b);border-bottom:1px solid var(--b);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.grade td.n,.grade th.n{text-align:right}
.grade td.nulo{color:var(--t3);font-style:italic;text-align:right}
.grade td.pk{color:var(--kw);text-align:right}
.grade td.ck,.grade th.ck{width:32px;padding:0;text-align:center}
.grade td.ativa{outline:2px solid var(--acc);outline-offset:-2px}
.grade td.sel{background:var(--sel)}
.caixa{display:inline-block;width:16px;height:16px;border-radius:5px;border:1px solid rgba(255,255,255,.35);vertical-align:middle}
.tp{display:inline-flex;align-items:center;justify-content:center;min-width:16px;height:16px;padding:0 2px;border-radius:4px;font-size:9px;font-weight:700;color:#fff;margin-right:6px;vertical-align:-3px;box-shadow:inset 0 1px 0 rgba(255,255,255,.35)}
.t-int{background:#2c6fdc}.t-real{background:#1d8181}.t-txt{background:#5f6b7a}.t-pk{background:#927010}.t-data{background:#c8443b}
.rolagem{position:absolute;right:2px;top:34px;width:6px;height:120px;border-radius:3px;background:rgba(255,255,255,.22)}
.prop{padding:6px 0;overflow:hidden}
.sec{display:flex;align-items:center;gap:6px;padding:8px 14px 6px;font-weight:600}
.sec .sub{color:var(--t3);font-weight:400}
.lin{display:flex;padding:3px 14px;gap:10px}
.lin .k{width:110px;color:var(--t2);flex:none}
.lin .v{color:var(--t);min-width:0;overflow:hidden;white-space:nowrap;text-overflow:ellipsis}
.div{height:1px;background:var(--b);margin:8px 0}
.seg{display:flex;margin:4px 10px 6px;padding:2px;border-radius:999px;background:rgba(0,0,0,.25);border:1px solid var(--fb)}
.seg span{flex:1;text-align:center;padding:4px 0;white-space:nowrap;border-radius:999px;color:var(--t2)}
.seg span.on{background:linear-gradient(var(--bt1),var(--bt2));color:var(--t);box-shadow:inset 0 1px 0 var(--btluz)}
.btn{height:28px;padding:0 15px;border-radius:999px;border:1px solid var(--btb);background:linear-gradient(var(--bt1),var(--bt2));color:var(--t);font:inherit;font-weight:600;display:inline-flex;align-items:center;gap:6px;box-shadow:inset 0 1px 0 var(--btluz),0 1px 2px rgba(0,0,0,.4)}
.btn.pri{color:#fff;border-color:#1d4d9c;background:linear-gradient(#5b93ee,#2c6fdc 25%,#2459b5)}
.u{display:inline-flex;align-items:center;justify-content:center;height:20px;min-width:30px;padding:0 6px;border-radius:999px;background:#f2b65e;color:#1b1c1f;font-size:11px;font-weight:800;box-shadow:0 0 0 2px #111214,0 2px 6px rgba(0,0,0,.5)}
.nota{color:#f2d29a;font-size:12px;line-height:1.45}
.legenda{display:flex;gap:10px;align-items:flex-start;padding:10px 14px;border-radius:10px;background:rgba(242,182,94,.10);border:1px solid rgba(242,182,94,.35)}
.cap{font-size:20px;font-weight:700;color:var(--t);margin:0}
.capsub{color:var(--t2);margin:4px 0 0;font-size:13px;line-height:1.5;max-width:1100px}
.cod{font-family:Consolas,"Cascadia Mono","DejaVu Sans Mono",monospace;font-size:12.5px}
.ic{width:16px;height:16px;fill:none;stroke:currentColor;stroke-width:1.6;stroke-linecap:round;stroke-linejoin:round;flex:none}
.hist{display:flex;align-items:flex-end;gap:2px;height:22px}
.hist i{display:block;width:7px;background:#5b93ee;border-radius:2px 2px 0 0}
"""

def svg(d):
    return f'<svg class="ic" viewBox="0 0 24 24" aria-hidden="true"><path d="{d}"></path></svg>'

IC = {
    "voltar": "M19 12H5m6-6-6 6 6 6",
    "avancar": "M5 12h14m-6-6 6 6-6 6",
    "pasta": "M3 7h6l2 2h10v9H3z",
    "arquivo": "M7 3h7l4 4v14H7z",
    "salvar": "M5 4h11l3 3v13H5zM8 4v5h7V4",
    "mais": "M12 5v14M5 12h14",
    "col": "M9 4v16M15 4v16M4 4h16v16H4z",
    "dup": "M8 8h11v11H8zM5 5h11",
    "lixo": "M5 7h14M10 7V4h4v3M7 7l1 13h8l1-13",
    "desf": "M9 7 4 12l5 5M4 12h11a5 5 0 0 1 0 10",
    "ref": "M15 7l5 5-5 5M20 12H9a5 5 0 0 0 0 10",
    "lupa": "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zm5 12 4 4",
    "filtro": "M4 5h16l-6 8v6l-4-2v-4z",
    "atual": "M20 11a8 8 0 1 0-2 6M20 5v6h-6",
    "play": "M7 5v14l11-7z",
    "stop": "M7 7h10v10H7z",
    "term": "M4 5h16v14H4zM7 9l3 3-3 3M12 15h5",
    "exp": "M12 3v12M7 8l5-5 5 5M5 15v5h14v-5",
    "painel": "M4 5h16v14H4zM14 5v14",
    "x": "M6 6l12 12M18 6 6 18",
    "seta": "M12 6l5 7H7z",
    "chev": "M6 9l6 6 6-6",
    "copiar": "M8 8h11v11H8zM5 16V5h11",
    "ok": "M5 12l4 4 10-10",
    "link": "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
}

def ib(nome, rot, off=False):
    return f'<button class="ib{" off" if off else ""}" aria-label="{rot}" title="{rot}">{svg(IC[nome])}</button>'

def titulo(nome, menus=("Arquivo", "Editar", "Exibir", "Banco", "Tabela", "Ajuda")):
    m = "".join(f'<span class="menu">{x}</span>' for x in menus)
    return (f'<div class="titulo"><span class="logo" aria-hidden="true"></span>{m}<span class="nome">{nome}</span>'
            f'<span class="jb">–</span><span class="jb">□</span><span class="jb">×</span></div>')

def barra(busca, liga=()):
    def b(n, r):
        return ib(n, r, off=n not in liga)
    return ('<div class="barra">' + b("voltar", "Voltar") + b("avancar", "Avançar") + '<span class="sep"></span>'
            + b("pasta", "Abrir pasta") + b("arquivo", "Abrir arquivo") + b("salvar", "Salvar") + '<span class="sep"></span>'
            + b("mais", "Novo") + b("col", "Nova coluna") + b("dup", "Duplicar") + b("lixo", "Excluir")
            + b("desf", "Desfazer") + b("ref", "Refazer")
            + f'<div class="busca">{svg(IC["lupa"])}<span style="flex:1">{busca}</span><span class="kbd">Ctrl+F</span></div>'
            + b("filtro", "Filtrar") + b("atual", "Atualizar") + '<span class="sep"></span>'
            + b("play", "Executar") + b("stop", "Interromper") + b("term", "Nova consulta") + b("exp", "Exportar")
            + '<span style="flex:1"></span>' + b("painel", "Painel") + '</div>')

def abas_doc(abas):
    h = '<div class="abas" role="tablist">'
    for txt, on in abas:
        h += f'<div class="aba{" on" if on else ""}" role="tab" aria-selected="{"true" if on else "false"}">{txt}<span class="x">×</span></div>'
    return h + f'<span class="aba" style="padding:0 8px;color:var(--t3)">+</span><span style="flex:1"></span></div>'

def explorador(item_on="clientes"):
    tabs = [("clientes", "60"), ("eventos", "5.000"), ("itens_pedido", "456"), ("notas", "2"), ("pedidos", "240"),
            ("produtos", "10"), ("Tabela com espaço", "4")]
    linhas = ""
    for n, q in tabs:
        on = ' style="background:rgba(44,111,220,.22);border-radius:6px;margin:0 6px;padding-left:30px"' if n == item_on else ' style="padding-left:36px"'
        linhas += f'<div class="lin"{on}><span class="v" style="flex:1">{n}</span><span style="color:var(--t3)">{q}</span></div>'
    return ('<div class="doca" style="width:258px;flex:none">'
            '<div class="abas"><div class="aba on" aria-label="Explorador">▤</div><div class="aba" aria-label="Histórico">◷</div></div>'
            '<div class="prop"><div class="lin" style="font-weight:600">⌄ exemplo.db</div>'
            '<div class="lin" style="padding-left:24px">⌄ Tabelas <span style="margin-left:auto;color:var(--t3)">7</span></div>'
            + linhas +
            '<div class="lin" style="padding-left:24px">› Visões <span style="margin-left:auto;color:var(--t3)">1</span></div>'
            '<div class="lin" style="padding-left:24px">› Sistema <span style="margin-left:auto;color:var(--t3)">7</span></div>'
            '</div></div>')

def u(n):
    return f'<span class="u">{n}</span>'

def pagina(titulo_pag, w, h, corpo, fundo="#111214"):
    return f'''<!doctype html>
<html lang="pt-BR">
<head>
<meta charset="utf-8">
<title>{titulo_pag}</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<style>{CSS}</style>
</helmet>
<div class="est" style="width:100%;min-height:{h}px;height:{h}px;overflow:hidden;position:relative;background:{fundo}">
{corpo}
</div>
</x-dc>
<script type="text/x-dc" data-dc-script data-props='{{"$preview":{{"width":{w},"height":{h}}}}}'>
class Component extends DCLogic {{
renderVals() {{
return {{}};
}}
}}
</script>
</body>
</html>
'''

# ---------------------------------------------------------------- Hoje
HOJE = [
    ("8e57f43c91a44114005ebeaaa4fb9551", 1440, 900, "Dados (claro): a grade crua e o rodapé formatado, a paginação",
     [("U1", 790, 168), ("U1", 620, 883), ("U2", 1000, 883)]),
    ("7ebaa9cddd67ebb51ce922feebe2477b", 1280, 800, "CSV filtrado: contagem em cima e embaixo, a seta da ordenação, o caminho cortado",
     [("U3", 825, 74), ("U3", 80, 783), ("U7", 686, 107), ("U6", 1180, 342)]),
    ("2b45cd043d56129c7d656eeb24dea1f0", 1440, 900, "Tabela vazia: sem cabeçalho, contagem dupla, paginação sem linhas",
     [("U5", 580, 128), ("U3", 1127, 76), ("U3", 58, 883)]),
    ("7599f2472987784866561d667c82f65d", 1280, 800, "JSON: a faixa repete o nome da aba; o caminho cortado nas Propriedades",
     [("U4", 345, 146), ("U6", 1180, 377)]),
    ("bd0908e7711b20161d15dd21d7678888", 1440, 900, "Consulta: a posição do cursor em cima (no JSON, embaixo)",
     [("U4", 1090, 146)]),
    ("63ba5c35dae00590a41fab1b2187acb6", 1440, 900, "Visão geral: faixa com o caminho inteiro; dois cliques por linha; Tipo quase vazio",
     [("U4", 760, 146), ("U8", 1083, 384), ("U8", 875, 352)]),
]

def hoje():
    W = 660
    cards = ""
    for blob, iw, ih, leg, marcas in HOJE:
        esc = W / iw
        hh = round(ih * esc)
        ms = "".join(
            f'<span class="u" style="position:absolute;left:{round(x*esc)-38}px;top:{round(y*esc)-10}px">{n}</span>'
            for n, x, y in marcas)
        cards += (f'<figure style="margin:0;display:flex;flex-direction:column;gap:8px">'
                  f'<div style="position:relative;width:{W}px;height:{hh}px;border-radius:10px;overflow:hidden;border:1px solid rgba(255,255,255,.14)">'
                  f'<img src="/_blob/{blob}" alt="{leg}" style="width:{W}px;height:{hh}px;display:block">{ms}</div>'
                  f'<figcaption style="color:var(--t2);font-size:13px">{leg}</figcaption></figure>')
    corpo = (f'<div style="padding:40px 48px;display:flex;flex-direction:column;gap:28px">'
             f'<div><p class="cap">Hoje, na 0.2.3: onde estão os achados</p>'
             f'<p class="capsub">Capturas dos testes de tela de 22/10/2026. Cada etiqueta âmbar é um achado de '
             f'<span class="cod">design/revisao-ui-0.3.md</span>; as pranchetas ao lado mostram a proposta.</p></div>'
             f'<div style="display:grid;grid-template-columns:repeat(2, minmax(0, 1fr));gap:36px 40px">{cards}</div></div>')
    return pagina("Hoje, na 0.2.3", 1440, 1560, corpo)

# ---------------------------------------------------------------- Grade
CLIENTES = [
    (1, "Lucas Brandão", "lucas.1@example.test", "Porto Alegre", "154,03", "2026-02-02 01:07:00"),
    (2, "Carla Ibiapina", "carla.2@example.test", "Recife", "14.542,56", "2026-03-03 02:14:00"),
    (3, "Bruno Brandão", "bruno.3@example.test", "Recife", "13.741,03", "2026-04-04 03:21:00"),
    (4, "Olívia Lacerda", "olívia.4@example.test", "Curitiba", "17.051", "2026-05-05 04:28:00"),
    (5, "Bruno Ibiapina", "bruno.5@example.test", "Porto Alegre", "4.321,49", "2026-06-06 05:35:00"),
    (6, "Rafael Ibiapina", "rafael.6@example.test", "São Paulo", "10.803,61", "2026-07-07 06:42:00"),
    (7, "Gabriela Guimarães", "gabriela.7@example.test", "Rio de Janeiro", None, "2026-08-08 07:49:00"),
    (8, "João Esteves", "joão.8@example.test", "Belo Horizonte", "2.024,53", "2026-09-09 08:56:00"),
    (9, "Lucas Otoni", "lucas.9@example.test", "São Paulo", "2.297,27", "2026-01-10 09:03:00"),
    (10, "Nuno Esteves", "nuno.10@example.test", "Curitiba", "4.112,96", "2026-02-11 10:10:00"),
    (12, "Ana Coutinho", "ana.12@example.test", "Rio de Janeiro", "14.200,04", "2026-04-13 12:24:00"),
    (13, "Rafael Otoni", "rafael.13@example.test", "São Paulo", "3.591,75", "2026-05-14 13:31:00"),
    (14, "Elisa Alvarenga", "elisa.14@example.test", "Recife", None, "2026-06-15 14:38:00"),
    (15, "Elisa Coutinho", "elisa.15@example.test", "Rio de Janeiro", "11.524,81", "2026-07-16 15:45:00"),
    (16, "Gabriela Alvarenga", "gabriela.16@example.test", "Recife", "12.763,07", "2026-08-17 16:52:00"),
    (17, "Diego Otoni", "diego.17@example.test", "Porto Alegre", "9.475,82", "2026-09-18 17:59:00"),
    (18, "Lucas Brandão", "lucas.18@example.test", "São Paulo", "6.672,99", "2026-01-19 18:06:00"),
    (19, "Quitéria Figueira", "quitéria.19@example.test", "Belo Horizonte", "16.472,81", "2026-02-20 19:13:00"),
    (20, "Gabriela Nogueira", "gabriela.20@example.test", "Porto Alegre", "17.810,34", "2026-03-21 20:20:00"),
    (21, "Karina Alvarenga", "karina.21@example.test", "Recife", None, "2026-04-22 21:27:00"),
    (23, "Tiago Damasceno", "tiago.23@example.test", "São Paulo", "7.276,51", "2026-06-24 23:41:00"),
    (24, "Quitéria Esteves", "quitéria.24@example.test", "Rio de Janeiro", "7.718,19", "2026-07-25 00:48:00"),
    (25, "Sofia Brandão", "sofia.25@example.test", "São Paulo", "12.422,62", "2026-08-26 01:55:00"),
    (26, "Paulo Junqueira", "paulo.26@example.test", "Porto Alegre", "14.290,75", "2026-09-27 02:02:00"),
]

def cabecalho_clientes(seta=True):
    s = f' <span style="color:var(--acc)">▲</span>' if seta else ""
    return ('<tr><th class="ck"><span class="caixa"></span></th>'
            '<th class="n" style="width:64px"><span class="tp t-pk">PK</span>id</th>'
            '<th style="width:170px"><span class="tp t-txt">Aa</span>nome</th>'
            '<th style="width:190px"><span class="tp t-txt">Aa</span>email</th>'
            '<th style="width:130px"><span class="tp t-txt">Aa</span>cidade</th>'
            f'<th class="n" style="width:110px"><span class="tp t-real">,5</span>limite{s}</th>'
            '<th class="n" style="width:76px"><span class="tp t-int">12</span>ativo</th>'
            '<th><span class="tp t-txt">Aa</span>criado_em</th></tr>')

def grade_clientes(n=24, sel=(1, 4)):
    linhas = ""
    for i, (id_, nome, email, cid, lim, cri) in enumerate(CLIENTES[:n]):
        if lim is None:
            lc = '<td class="nulo">NULL</td>'
        else:
            cls = "n"
            if sel[0] <= i <= sel[1]:
                cls += " sel"
            if i == sel[0]:
                cls += " ativa"
            lc = f'<td class="{cls}">{lim}</td>'
        linhas += (f'<tr><td class="ck"><span class="caixa"></span></td><td class="pk">{id_}</td><td>{nome}</td>'
                   f'<td>{email}</td><td>{cid}</td>{lc}<td class="n">1</td><td>{cri}</td></tr>')
    return f'<div class="grade"><table>{cabecalho_clientes()}{linhas}</table><span class="rolagem" aria-hidden="true"></span></div>'

def propriedades_colunas():
    cols = [("PK", "t-pk", "id", "INTEGER"), ("Aa", "t-txt", "nome", "TEXT"), ("Aa", "t-txt", "email", "TEXT"),
            ("Aa", "t-txt", "cidade", "TEXT"), (",5", "t-real", "limite", "REAL"), ("12", "t-int", "ativo", "INTEGER"),
            ("Aa", "t-txt", "criado_em", "TEXT")]
    h = ""
    for g, c, n, t in cols:
        on = ' style="background:rgba(44,111,220,.22);border-radius:6px;margin:0 6px;padding:5px 8px"' if n == "limite" else ' style="padding:5px 14px"'
        h += f'<div class="lin"{on}><span class="tp {c}">{g}</span><span class="v" style="flex:1;font-weight:600">{n}</span><span style="color:var(--t3);font-size:11px">{t}</span></div>'
    return h

def doca_direita(objeto, seg_on, corpo):
    segs = "".join(f'<span class="{"on" if s == seg_on else ""}">{s}</span>' for s in ("Resumo", "Colunas", "Perfil", "Índices", "SQL"))
    return ('<div class="doca" style="width:272px;flex:none">'
            f'<div class="abas"><div class="aba on">▣ Propriedades <span class="sub">{objeto}</span></div></div>'
            f'<div class="seg">{segs}</div><div class="prop">{corpo}</div></div>')

def grade_board():
    faixa = ('<div class="faixa"><span class="rot">Filtros</span>'
             '<span class="pilula"><b>ativo</b> igual a <b>1</b> ×</span>'
             '<span style="color:var(--t2);margin-left:6px">+ <b style="color:var(--t)">Condição</b></span>'
             '<span style="flex:1"></span><span class="lnk">Limpar filtros</span></div>')
    rodape = ('<div class="rodape">' + u("U3") + '<span>Linhas 1–24 de 55</span><span class="ponto">·</span>'
              '<span style="color:var(--t3)">filtradas de 60</span>'
              '<span class="mais">Contagem 4 · Soma 49.656,08 · Média 12.414,02 · Mín. 4.321,49 · Máx. 17.051</span></div>')
    doc = (f'<div class="coluna" style="flex:1">{abas_doc([('exemplo.db <span class="sub">Visão geral</span>', False), ("clientes", True), ("Consulta 1", False)])}'
           f'<div class="doc">{faixa}{grade_clientes()}{rodape}</div></div>')
    col = (propriedades_colunas() + '<div class="div"></div>'
           '<div class="sec">Números na grade</div>'
           '<div class="lin"><span class="v" style="color:var(--t2);white-space:normal">Formatados no idioma da interface, sem arredondar. '
           'O Copiar, a edição e a ficha levam o valor gravado (14542.56).</span></div>')
    janela = (f'<div class="janela" style="height:900px">{titulo("exemplo.db · DataBox")}'
              f'{barra("Buscar em clientes", liga=("voltar","pasta","arquivo","mais","dup","lixo","filtro","atual","term","exp","painel"))}'
              f'<div class="corpo">{explorador()}{doc}{doca_direita("clientes", "Colunas", col)}</div></div>')
    marcas = (f'<span class="u" style="position:absolute;left:836px;top:226px">U1</span>'
              f'<span class="u" style="position:absolute;left:1018px;top:133px">U3</span>'
              f'<span class="u" style="position:absolute;left:1108px;top:330px">U2</span>')
    return pagina("Uma grade só", 1440, 900, janela + marcas)

# ---------------------------------------------------------------- Vazia
def vazia_board():
    def doc(titulo_doc, faixa, msg, sub, botao, rodape):
        cab = ('<tr><th class="ck"><span class="caixa"></span></th><th class="n" style="width:70px"><span class="tp t-pk">PK</span>id</th>'
               '<th style="width:180px"><span class="tp t-txt">Aa</span>descricao</th><th class="n" style="width:120px"><span class="tp t-real">,5</span>valor</th>'
               '<th><span class="tp t-data">31</span>criado_em</th></tr>')
        return (f'<div style="display:flex;flex-direction:column;gap:10px;flex:1;min-width:0">'
                f'<div style="color:var(--t2);font-size:13px">{titulo_doc}</div>'
                f'<div class="doc" style="height:430px;flex:none">{faixa}'
                f'<div class="grade" style="flex:none"><table>{cab}</table></div>'
                f'<div style="flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px">'
                f'<div style="font-size:13px;font-weight:600">{msg}</div><div style="color:var(--t3)">{sub}</div>{botao}</div>'
                f'<div class="rodape">{rodape}</div></div></div>')
    f1 = '<div class="faixa"><span class="rot">Filtros</span><span style="color:var(--t2)">+ <b style="color:var(--t)">Condição</b></span></div>'
    f2 = ('<div class="faixa"><span class="rot">Filtros</span><span class="pilula"><b>valor</b> maior que <b>1.000</b> ×</span>'
          '<span style="flex:1"></span><span class="lnk">Limpar filtros</span></div>')
    a = doc("Tabela sem linhas (extra_03)", f1, "Nenhuma linha em extra_03", "4 colunas, nenhuma linha.",
            f'<button class="btn pri">{svg(IC["mais"])}Nova linha</button>', "0 linhas")
    b = doc("Filtro que não acha nada", f2, "Nenhuma linha com estes filtros", "312 linhas ficaram de fora.",
            '<button class="btn">Limpar filtros</button>', "0 linhas · filtradas de 312")
    corpo = (f'<div style="padding:36px 48px;display:flex;flex-direction:column;gap:20px">'
             f'<div><p class="cap">{u("U5")} Vazio com as colunas à vista</p>'
             f'<p class="capsub">O cabeçalho fica, com os nomes e as pastilhas de tipo; o vazio vai no corpo, com a ação num botão. '
             f'Vale para os Dados, o CSV e o resultado vazio da Consulta. Sem seletor de página no rodapé (U2).</p></div>'
             f'<div style="display:flex;gap:40px">{a}{b}</div></div>')
    return pagina("Vazio com as colunas", 1440, 620, f'<div class="est" style="height:620px">{corpo}</div>')

# ---------------------------------------------------------------- Faixas
JSON_LINHAS = [
    '{', '  <span style="color:var(--chave)">"nome"</span>: <span style="color:var(--st)">"Painel de vendas"</span>,',
    '  <span style="color:var(--chave)">"versao"</span>: <span style="color:var(--st)">"2.4.1"</span>,',
    '  <span style="color:var(--chave)">"conexoes"</span>: [', '    {',
    '      <span style="color:var(--chave)">"nome"</span>: <span style="color:var(--st)">"principal"</span>,',
    '      <span style="color:var(--chave)">"arquivo"</span>: <span style="color:var(--st)">"C:\\\\dados\\\\vendas.db"</span>,',
    '      <span style="color:var(--chave)">"somente_leitura"</span>: <span style="color:var(--kw)">false</span>,',
    '      <span style="color:var(--chave)">"tempo_limite"</span>: <span style="color:var(--nu)">30</span>', '    }', '  ],',
    '  <span style="color:var(--chave)">"atalhos"</span>: <span style="color:var(--kw)">null</span>', '}']

def editor(linhas, ativa):
    nums = "".join(f'<div>{i+1}</div>' for i in range(len(linhas)))
    txt = "".join(
        f'<div style="{"background:rgba(255,255,255,.05);" if i == ativa else ""}white-space:pre">{l}</div>'
        for i, l in enumerate(linhas))
    return (f'<div class="cod" style="flex:1;min-height:0;display:flex;line-height:20px">'
            f'<div style="width:44px;text-align:right;padding:8px 10px 0 0;color:var(--t3);background:rgba(0,0,0,.12)">{nums}</div>'
            f'<div style="flex:1;padding:8px 0 0 14px">{txt}</div></div>')

def faixas_board():
    json_faixa = ('<div class="faixa">' + u("U4") +
                  '<span style="color:var(--t3)">raiz</span><span class="ponto">›</span><span style="color:var(--t3)">conexoes</span>'
                  '<span class="ponto">›</span><span style="color:var(--t3)">[0]</span><span class="ponto">›</span><b>arquivo</b>'
                  f'<span style="flex:1"></span><span style="color:var(--ok)" title="JSON válido">{svg(IC["ok"])}</span>'
                  '<button class="btn" style="height:26px">Formatar</button></div>')
    json_rod = ('<div class="rodape"><span>13 linhas</span><span class="ponto">·</span><span>Espaços: 2</span>'
                '<span class="mais">Linha 7, coluna 18</span></div>')
    json_doc = (f'<div class="coluna" style="flex:1">{abas_doc([("config.json", True)])}'
                f'<div class="doc" style="height:560px;flex:none">{json_faixa}{editor(JSON_LINHAS, 6)}{json_rod}</div></div>')
    sql = ['<span style="color:var(--kw)">SELECT</span> v.vendedor, <span style="color:var(--kw)">sum</span>(v.valor) <span style="color:var(--kw)">AS</span> total, c.cidade',
           '<span style="color:var(--kw)">FROM</span> arquivos.vendas_2025 v',
           '<span style="color:var(--kw)">JOIN</span> clientes c <span style="color:var(--kw)">ON</span> c.nome = v.vendedor',
           '<span style="color:var(--kw)">GROUP BY</span> v.vendedor, c.cidade',
           '<span style="color:var(--kw)">ORDER BY</span> total <span style="color:var(--kw)">DESC</span>;']
    q_faixa = ('<div class="faixa">' + u("U4") + '<span class="rot">Fontes</span>'
               '<span class="pilula neutra">▦ exemplo.db</span>'
               '<span class="pilula neutra"><span class="tp" style="background:#27854c">CSV</span>vendas_2025.csv <span style="color:var(--t3)">· arquivos.vendas_2025</span> ×</span>'
               '<span style="color:var(--t2)">+ <b style="color:var(--t)">Arquivo</b></span></div>')
    res = ('<div class="grade" style="flex:none;border-top:1px solid var(--b)"><table>'
           '<tr><th><span class="tp t-txt">Aa</span>vendedor</th><th class="n"><span class="tp t-real">,5</span>total</th><th><span class="tp t-txt">Aa</span>cidade</th></tr>'
           '<tr><td>Ana Coutinho</td><td class="n">48.211,40</td><td>Rio de Janeiro</td></tr>'
           '<tr><td>Bruno Brandão</td><td class="n">41.907,15</td><td>Recife</td></tr>'
           '<tr><td>Carla Ibiapina</td><td class="n">39.554,80</td><td>Recife</td></tr>'
           '<tr><td>João Esteves</td><td class="n">33.120,05</td><td>Belo Horizonte</td></tr></table></div>')
    q_rod = ('<div class="rodape"><span>4 linhas · 38 ms</span><span class="mais">Linha 5, coluna 22</span></div>')
    q_doc = (f'<div class="coluna" style="flex:1">{abas_doc([("Consulta 1", True)])}'
             f'<div class="doc" style="height:560px;flex:none">{q_faixa}<div style="height:150px;display:flex">{editor(sql, 4)}</div>{res}<div style="flex:1"></div>{q_rod}</div></div>')
    regra = ('<div class="legenda"><span class="u">U4</span><div class="nota"><b>Regra proposta para o guia.</b> A faixa de cima só existe '
             'para controle ou contexto que a aba não diz (filtros, a migalha do JSON, as fontes da consulta). Posição do cursor e '
             'contagens moram no rodapé, à direita. O TXT e a Visão geral ficam sem faixa; o caminho inteiro vai na dica de toda aba.</div></div>')
    corpo = (f'<div style="padding:36px 48px;display:flex;flex-direction:column;gap:20px">'
             f'<div><p class="cap">Faixas e rodapés com uma regra só</p>'
             f'<p class="capsub">À esquerda, o JSON sem o nome repetido: a faixa é a migalha do ponto do cursor. À direita, a Consulta da P4 '
             f'(bancada com ATTACH): a faixa mostra as fontes, com "+ Arquivo"; "Linha, coluna" desce para o rodapé, como no JSON.</p></div>'
             f'{regra}<div style="display:flex;gap:40px">{json_doc}{q_doc}</div></div>')
    return pagina("Faixas e rodapés", 1440, 900, corpo)

# ---------------------------------------------------------------- Perfil
PERFIL = [
    ("PK", "t-pk", "id", "INTEGER", "60", "0", "60", "1", "60", "30,5", "—", [6, 6, 6, 6, 6, 6, 6, 6, 6, 6]),
    ("Aa", "t-txt", "nome", "TEXT", "60", "0", "54", "Ana Coutinho", "Tiago Damasceno", "—", "Lucas Brandão (3)", None),
    ("Aa", "t-txt", "email", "TEXT", "60", "0", "60", "ana.12@…", "tiago.23@…", "—", "— (todos distintos)", None),
    ("Aa", "t-txt", "cidade", "TEXT", "60", "0", "8", "Belo Horizonte", "São Paulo", "—", "São Paulo (14)", None),
    (",5", "t-real", "limite", "REAL", "57", "3 (5%)", "57", "154,03", "19.437,39", "9.812,40", "—", [3, 5, 9, 12, 16, 18, 14, 10, 6, 4]),
    ("12", "t-int", "ativo", "INTEGER", "60", "0", "2", "0", "1", "0,92", "1 (55)", [2, 0, 0, 0, 0, 0, 0, 0, 0, 20]),
    ("Aa", "t-txt", "criado_em", "TEXT", "60", "0", "60", "2026-01-10", "2026-09-27", "—", "—", None),
]

def perfil_board():
    linhas = ""
    for g, c, n, t, pre, nul, dis, mn, mx, med, freq, hist in PERFIL:
        hh = ('<div class="hist">' + "".join(f'<i style="height:{max(2, v)}px"></i>' for v in hist) + '</div>') if hist else '<span style="color:var(--t3)">—</span>'
        on = ' style="background:rgba(44,111,220,.14)"' if n == "limite" else ""
        nulc = ' style="color:var(--aviso)"' if nul != "0" else ""
        linhas += (f'<tr{on}><td><span class="tp {c}">{g}</span><b>{n}</b></td>'
                   f'<td class="n">{pre}</td><td class="n"{nulc}>{nul}</td><td class="n">{dis}</td><td class="n">{mn}</td>'
                   f'<td class="n">{mx}</td><td class="n">{med}</td><td>{freq}</td><td>{hh}</td></tr>')
    tabela = ('<div class="grade"><table><tr>'
              '<th style="width:130px">Coluna</th><th class="n" style="width:86px">Preenchidas</th>'
              '<th class="n" style="width:66px">Nulos</th><th class="n" style="width:72px">Distintos</th><th class="n" style="width:104px">Mínimo</th>'
              '<th class="n" style="width:104px">Máximo</th><th class="n" style="width:76px">Média</th><th style="width:130px">Mais frequente</th>'
              f'<th>Distribuição</th></tr>{linhas}</table></div>')
    faixa = ('<div class="faixa"><span style="color:var(--t2)">clientes · 60 linhas · 7 colunas</span>'
             '<span style="flex:1"></span><span style="color:var(--t3)">Exato (até 100 mil valores; acima, ≈)</span>'
             f'<button class="btn" style="height:26px">{svg(IC["atual"])}Recalcular</button>'
             f'<button class="btn" style="height:26px">{svg(IC["exp"])}Exportar o perfil</button></div>')
    rodape = '<div class="rodape"><span>7 colunas</span><span class="ponto">·</span><span>calculado em 0,2 s</span></div>'
    doc = (f'<div class="coluna" style="flex:1">{abas_doc([("clientes", False), ('Perfil <span class="sub">clientes</span>', True)])}'
           f'<div class="doc">{faixa}{tabela}{rodape}</div></div>')
    vista = ('<div class="sec"><span class="tp t-real">,5</span>limite <span class="sub">REAL</span></div>'
             '<div class="lin"><span class="k">Preenchidas</span><span class="v">57 de 60 (95%)</span></div>'
             '<div class="lin"><span class="k">Nulos</span><span class="v" style="color:var(--aviso)">3</span></div>'
             '<div class="lin"><span class="k">Distintos</span><span class="v">57</span></div>'
             '<div class="lin"><span class="k">Mínimo</span><span class="v">154,03</span></div>'
             '<div class="lin"><span class="k">Mediana</span><span class="v">10.104,22</span></div>'
             '<div class="lin"><span class="k">Média</span><span class="v">9.812,40</span></div>'
             '<div class="lin"><span class="k">Desvio</span><span class="v">5.403,77</span></div>'
             '<div class="lin"><span class="k">Máximo</span><span class="v">19.437,39</span></div>'
             '<div class="div"></div><div class="sec">Distribuição <span class="sub">10 faixas</span></div>'
             '<div style="padding:4px 14px"><div class="hist" style="height:70px;gap:4px">'
             + "".join(f'<i style="height:{v*4}px;width:20px"></i>' for v in [3, 5, 9, 12, 16, 18, 14, 10, 6, 4]) +
             '</div><div style="display:flex;justify-content:space-between;color:var(--t3);font-size:11px;margin-top:4px"><span>0</span><span>10.000</span><span>20.000</span></div></div>'
             '<div class="div"></div><div class="lin"><span class="v" style="color:var(--t2);white-space:normal">Clique numa linha do perfil para ver a coluna aqui (Alt+5).</span></div>')
    janela = (f'<div class="janela" style="height:900px">{titulo("exemplo.db · DataBox")}'
              f'{barra("Buscar no perfil", liga=("voltar","pasta","arquivo","atual","exp","painel"))}'
              f'<div class="corpo">{explorador()}{doc}{doca_direita("limite", "Perfil", vista)}</div></div>')
    return pagina("Perfil da tabela", 1440, 900, janela)

# ---------------------------------------------------------------- Detalhes
def cartao(num, tit, antes, depois, w=640, ra='Hoje', rd='Proposta'):
    return (f'<div style="width:{w}px;display:flex;flex-direction:column;gap:12px;padding:20px;border-radius:12px;background:var(--painel);border:1px solid var(--b)">'
            f'<div style="display:flex;align-items:center;gap:10px">{u(num)}<b style="font-size:14px">{tit}</b></div>'
            f'<div style="display:grid;grid-template-columns:repeat(2, minmax(0, 1fr));gap:14px">'
            f'<div><div class="rot" style="margin-bottom:6px">{ra}</div>{antes}</div>'
            f'<div><div class="rot" style="margin-bottom:6px;color:var(--ok)">{rd}</div>{depois}</div></div></div>')

def mini_prop(linhas):
    return ('<div style="background:var(--c);border:1px solid var(--b);border-radius:10px;padding:6px 0">'
            + "".join(f'<div class="lin"><span class="k" style="width:90px">{k}</span><span class="v">{v}</span></div>' for k, v in linhas)
            + '</div>')

def detalhes_board():
    c6 = cartao("U6", "Caminhos cortados guardam o nome e a pasta-mãe",
                mini_prop([("Caminho", "vendas_2025.c…"), ("Caminho", "/h…s/config.json"), ("Caminho", "/…1/exemplo.db")]),
                mini_prop([("Caminho", "…/dados/vendas_2025.csv"), ("Caminho", "…/amostras/config.json"), ("Caminho", "…/dados/exemplo.db")]))
    th_a = ('<div class="grade" style="border:1px solid var(--b);border-radius:10px"><table><tr>'
            '<th class="n"><span style="color:var(--acc);float:left">▲</span><span class="tp t-int">12</span>quantidade</th></tr>'
            '<tr><td class="n">1</td></tr><tr><td class="n">2</td></tr></table></div>')
    th_d = ('<div class="grade" style="border:1px solid var(--b);border-radius:10px"><table><tr>'
            '<th class="n"><span class="tp t-int">12</span>quantidade <span style="color:var(--acc)">▲</span></th></tr>'
            '<tr><td class="n">1</td></tr><tr><td class="n">2</td></tr></table></div>')
    c7 = cartao("U7", "A seta da ordenação junto do nome", th_a, th_d)
    vg_a = ('<div class="grade" style="border:1px solid var(--b);border-radius:10px"><table>'
            '<tr><th>Nome</th><th class="n">Linhas</th><th>Tipo</th><th></th></tr>'
            '<tr><td style="color:var(--kw)">notas</td><td class="n">2</td><td><span class="pilula neutra" style="height:18px;font-size:10px">VIRTUAL</span></td><td style="color:var(--kw)">Colunas</td></tr>'
            '<tr><td style="color:var(--kw)">pedidos</td><td class="n">240</td><td></td><td style="color:var(--kw)">Colunas</td></tr></table></div>')
    vg_d = ('<div class="grade" style="border:1px solid var(--b);border-radius:10px"><table>'
            '<tr><th style="width:150px">Nome</th><th class="n">Linhas</th><th class="n">Índices</th><th style="width:40px"></th></tr>'
            '<tr><td>notas <span class="pilula neutra" style="height:18px;font-size:10px">VIRTUAL</span></td><td class="n">2</td><td class="n">0</td><td></td></tr>'
            f'<tr style="background:rgba(255,255,255,.05)"><td>pedidos</td><td class="n">240</td><td class="n">2</td><td>{ib("col", "Colunas de pedidos")}</td></tr></table></div>')
    c8 = cartao("U8", "Visão geral: a linha abre a tabela", vg_a, vg_d)
    rod_a = '<div class="rodape" style="border-radius:10px;font-size:11px;padding:8px;height:auto;white-space:normal">Vendas › E6 · Somente leitura · 1.204 linhas · 6 colunas · A1:F1204 · 1 mesclada</div>'
    rod_d = '<div class="rodape" style="border-radius:10px;font-size:11px;padding:8px;height:auto;white-space:normal">Vendas › E6 · Somente leitura · 1.204 linhas · 6 colunas</div>'
    c9 = cartao("U9", "Rodapé da planilha enxuto", rod_a, rod_d)
    menu = lambda itens: ('<div style="background:rgba(30,31,35,.96);border:1px solid var(--fb);border-radius:12px;padding:6px">'
                          + "".join(f'<div style="display:flex;align-items:center;gap:8px;padding:5px 8px;color:{c}"><span style="width:16px">{i}</span>{t}</div>' for i, t, c in itens)
                          + '</div>')
    m_a = menu([("", "Copiar", "var(--t)"), ("", "Colar", "var(--t)"), (svg(IC["filtro"]), "Filtrar por este valor", "var(--t)"),
                ("", "Marcar as linhas", "var(--t)"), (svg(IC["lixo"]), "Excluir as linhas", "var(--perigo)")])
    m_d = ('<div class="nota" style="font-size:12px">Sem mudança no desenho: <b>o ícone aparece só quando o mesmo verbo tem botão na '
           'barra de ferramentas</b> (Filtrar, Excluir, Abrir pasta, Salvar). O menu ensina a barra. Hoje já é quase assim; '
           'falta escrever no guia.</div>')
    c11 = cartao("U11", "Ícones nos menus: regra a registrar", m_a, m_d)
    pref = ('<div style="background:var(--c);border:1px solid var(--b);border-radius:10px;padding:14px;display:flex;flex-direction:column;gap:12px">'
            '<div style="display:flex;flex-direction:column;gap:6px"><span style="color:var(--t2)">Tema</span><span class="seg" style="margin:0;flex:1"><span>Sistema</span><span class="on">Escuro</span><span>Claro</span></span></div>'
            '<div style="display:flex;flex-direction:column;gap:6px"><span style="color:var(--t2)">Idioma</span><span class="busca" style="margin:0;flex:1;width:auto;color:var(--t)">Seguir o sistema <span style="margin-left:auto">⌄</span></span></div>'
            '<div style="display:flex;flex-direction:column;gap:6px"><span style="color:var(--t2)">Números na grade</span><span class="seg" style="margin:0;flex:1"><span class="on">Formatados</span><span>Como gravados</span></span></div></div>')
    pseudo = ('<div style="display:flex;flex-direction:column;gap:8px">'
              '<button class="btn pri" style="align-self:flex-start">[Ábrír pástá·······]</button>'
              '<div class="busca" style="margin:0;width:250px"><span style="overflow:hidden;white-space:nowrap;text-overflow:ellipsis">[Búscár ém clíéntés········]</span></div>'
              '<div class="nota">Pseudoidioma da P7 (40% mais longo): nada com largura fixa pelo português.</div></div>')
    c12 = cartao("P7", "Preferências › Aparência: idioma e números", pseudo, pref, ra="Pseudoidioma", rd="Preferências")
    corpo = (f'<div style="padding:36px 48px;display:flex;flex-direction:column;gap:24px">'
             f'<div><p class="cap">Detalhes: ganhos rápidos e regras</p>'
             f'<p class="capsub">Achados de esforço pequeno. U6 a U9 cabem numa 0.2.4 de acabamento ou no começo da 0.3; '
             f'a U11 é só texto no guia; o cartão P7 junta o idioma (já decidido) e a opção dos números da U1.</p></div>'
             f'<div style="display:flex;flex-wrap:wrap;gap:24px">{c6}{c7}{c8}{c9}{c11}{c12}</div></div>')
    return pagina("Detalhes", 1440, 880, corpo)

ARQS = {
    "Propostas03Hoje.dc.html": hoje,
    "Propostas03Grade.dc.html": grade_board,
    "Propostas03Vazia.dc.html": vazia_board,
    "Propostas03Faixas.dc.html": faixas_board,
    "Propostas03Perfil.dc.html": perfil_board,
    "Propostas03Detalhes.dc.html": detalhes_board,
}

if __name__ == "__main__":
    os.makedirs(RAIZ, exist_ok=True)
    for nome, f in ARQS.items():
        with open(os.path.join(RAIZ, nome), "w", encoding="utf-8") as s:
            s.write(f())
        print(nome)
