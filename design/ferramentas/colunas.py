"""Prancheta 16: copiar, colar e trazer colunas (escuro) (17/10/2026).
Cinco cenas, cada uma com título e legenda, como na prancheta 15:
- (a) o menu do cabeçalho do CSV com os itens novos (Copiar coluna, Colar como nova coluna, Colar sobre esta coluna,
  Colar com correspondência, Trazer colunas de outro arquivo) antes dos de hoje, com a coluna valor selecionada;
- (b) a cola com filtro ligado: os dois Colar desligados, com a dica;
- (c) as quantidades diferentes: o aviso compacto no topo do documento e, depois de Colar 1.180, o cabeçalho da coluna
  colada em edição com o nome sugerido e o aviso flutuante;
- (d) a janela Trazer colunas preenchida (de onde, casar as linhas, o medidor de pares, as três opções, as chaves
  repetidas, o que trazer e a prévia);
- (e) depois de trazer: o aviso flutuante com Marcar as linhas sem par e a grade com as colunas novas.
Também guarda a silhueta nova trazer_colunas (duas colunas com uma seta entre elas) na biblioteca (prancheta 9, seção
Silhuetas da barra de ferramentas), se ela ainda não estiver lá, e põe a prancheta no canvas.json.

Usa a folha de estilo, as definições e o script do Estados.dc.html (que já tem as regras das outras telas e as
camadas de 16 e 17/10), mais as regras de formulário do Janelas.dc.html. Os ícones e os botões vêm das próprias
pranchetas, pelo aria-label ou pelo texto, como no estados_novos.py. Refaz a prancheta inteira; recusa gravar por
cima de um Colunas.dc.html que não tenha a marca dele, uma silhueta trazer_colunas diferente na biblioteca e um
lugar no canvas.json ocupado por outra prancheta.
Uso: python -I -X utf8 colunas.py [pasta das pranchetas]"""
import json
import re
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
PASTA = Path(sys.argv[1]) if len(sys.argv) > 1 else AQUI.parent / "canvas"
sys.stdout.reconfigure(encoding="utf-8")
UM_SVG = r'<svg\b[^>]*>(?:(?!</svg>).)*</svg>'
MARCA = "/* prancheta 16: copiar, colar e trazer colunas (colunas.py) */"
ALTURA = 2740
LUGAR = {"x": 0, "y": 8300, "w": 1440}
TITULO = "16 · Copiar, colar e trazer colunas (escuro)"


def ler(nome):
    return (PASTA / f"{nome}.dc.html").read_text(encoding="utf-8")


def achar(padrao, texto, onde, grupo=1):
    m = re.search(padrao, texto, re.S)
    if not m:
        raise SystemExit(f"colunas: {onde}: não achei {padrao[:90]!r}")
    return m.group(grupo)


def botao(texto, aria, onde):
    return achar(r'(<button class="ib[^"]*"[^>]*aria-label="' + re.escape(aria) + r'"[^>]*>' + UM_SVG + '</button>)', texto, onde)


def svg_de(texto, aria, onde):
    return achar(r'<button class="ib[^"]*"[^>]*aria-label="' + re.escape(aria) + r'"[^>]*>(' + UM_SVG + ')</button>', texto, onde)


def lado(svg, n):
    return re.sub(r'width="[^"]+" height="[^"]+"', f'width="{n}" height="{n}"', svg, count=1)


def estilo(texto):
    return texto[texto.index("<style>") + len("<style>"):texto.index("</style>")].strip("\n").split("\n")


estados, csv, janelas, dados, json_, claro, icones = (ler(n) for n in ("Estados", "Csv", "Janelas", "Dados", "Json", "DadosClaro", "Icones"))

# --- a silhueta nova: duas colunas (a de origem em .6) e a seta que leva da primeira para a segunda -----------------
TRAZER_D = ('<path d="M4 3h2.4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zM3.1 8.4v1.3h4.2V8.4zM3.1 14.3v1.3h4.2v-1.3z" '
            'style="fill-rule:evenodd;opacity:.6"></path>'
            '<path d="M17.6 3H20a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-2.4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zM16.7 8.4v1.3h4.2V8.4zM16.7 14.3v1.3h4.2v-1.3z" '
            'style="fill-rule:evenodd"></path>'
            '<path d="M9.4 10.9h2V8.8l3.2 3.2-3.2 3.2v-2.1h-2z"></path>')
TRAZER = f'<svg class="sil" viewBox="0 0 24 24">{TRAZER_D}</svg>'

# biblioteca: o ladrilho no fim da seção das silhuetas
sec = re.search(r'(<section class="painel" aria-label="Silhuetas da barra de ferramentas">.*?)(</div></section>)', icones, re.S)
if not sec:
    raise SystemExit("colunas: Icones.dc.html: não achei a seção Silhuetas da barra de ferramentas")
ladrilho = ('<div class="tile"><div class="tamanhos">'
            + "".join(f'<span class="sil-v"><svg class="sil" viewBox="0 0 24 24" width="{n}" height="{n}" aria-hidden="true">{TRAZER_D}</svg></span>'
                      for n in (18, 14))
            + '</div><div class="nome">Trazer colunas</div><div class="ext">barra do CSV, menu do cabeçalho e Propriedades › Coluna (nova)</div></div>')
if '<div class="nome">Trazer colunas</div>' in sec.group(1):
    if ladrilho not in sec.group(1):
        raise SystemExit("colunas: a biblioteca já tem um ladrilho Trazer colunas diferente; não mexi em nada")
    icones_novo = icones
else:
    icones_novo = icones[:sec.end(1)] + ladrilho + icones[sec.end(1):]

# --- folha: a do Estados + as regras de formulário do Janelas + as desta prancheta ----------------------------------
cabeca = estados[: estados.index("</helmet>")]
cabeca = re.sub(r"<title>.*?</title>", "<title>Copiar, colar e trazer colunas</title>", cabeca, count=1)
ja = set(estilo(estados))
extra = [l for l in estilo(janelas) if l.startswith((".cpo{", ".grade2", ".campo.pq", ".btn.pq{")) and l not in ja]
PROPRIO = MARCA + """
.quadro{grid-template-columns:repeat(2,minmax(0,1fr))}
.palco .grade th.selcol{background:var(--sel);color:var(--t)}
.grade td.selcol{background:var(--tint)}
.menu-ctx.largo{width:300px}
.mi.off .at{color:var(--t3)}
.mi .vz{width:14px;flex:none}
.dica-col{position:absolute;width:212px;padding:7px 11px;border-radius:8px;font-size:11px;line-height:16px;color:var(--t);white-space:normal;text-wrap:pretty}
.etapa{display:flex;flex-direction:column;gap:8px;min-width:0}
.etapas{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:18px}
.no-doc{padding:12px 14px;border-bottom:1px solid var(--b);background:var(--c)}
.aviso-j .corpo{display:flex;flex-direction:column;gap:8px;min-width:0}
.aviso-j .acoes{display:flex;align-items:center;gap:6px;flex-wrap:wrap}
.aviso-j .acoes .rot-sel{color:var(--t2);margin-right:2px}
.toast.com-acao{padding:5px 6px 5px 16px;gap:10px}
.dlg.tc{width:640px}
.tc .corpo-tc{padding:14px 20px 16px;display:flex;flex-direction:column;gap:14px}
.tc .bloco{display:flex;flex-direction:column;gap:8px;min-width:0}
.tc .cab-bloco{display:flex;align-items:center;gap:8px;min-height:24px}
.tc .meta{color:var(--t3);font-size:11.5px}
.tc .casar{display:grid;grid-template-columns:minmax(0,1fr) 20px minmax(0,1fr);gap:4px 8px;align-items:center}
.tc .casar .igual{text-align:center;color:var(--t2);font-weight:600}
.tc .casar .meta{padding-left:2px}
.tc .casar>.campo{display:flex;width:100%}
.tc .aviso-j .campo{display:inline-flex;width:auto}
.medida{display:flex;flex-direction:column;gap:6px;margin-top:4px}
.medida .txt{display:flex;align-items:baseline;gap:6px;color:var(--t2)}
.medida .txt strong{color:var(--t);font-weight:600;font-variant-numeric:tabular-nums}
.medidor{height:8px;border-radius:999px;background:var(--f);border:1px solid var(--fb);overflow:hidden;box-shadow:inset 0 1px 2px rgba(0,0,0,.35)}
.medidor>div{height:100%;border-radius:999px;background:var(--textura),var(--volpri);box-shadow:inset 0 1px 0 rgba(255,255,255,.35),0 0 10px var(--brilhof)}
.opcoes{display:flex;flex-wrap:wrap;gap:8px}
.tg{height:24px;display:inline-flex;align-items:center;gap:6px;padding:0 11px;border-radius:999px;border:1px solid var(--fbc);background:transparent;color:var(--t2);font:inherit;white-space:nowrap;cursor:pointer;transition:box-shadow .2s ease}
.tg .ic{width:12px;height:12px;stroke-width:2.2;margin-left:-2px}
.tg:hover{color:var(--t);box-shadow:0 0 12px var(--brilho)}
.tg[aria-pressed=true]{color:var(--t);border-color:color-mix(in oklab,var(--acc) 45%,transparent);background:var(--tint);box-shadow:inset 0 1px 0 rgba(255,255,255,.08),0 1px 3px rgba(0,0,0,.25)}
.tg[aria-pressed=true] .ic{color:var(--acct)}
.lista-tr{position:relative;border-radius:10px;border:1px solid var(--fb);background:var(--f);box-shadow:inset 0 1px 2px rgba(0,0,0,.3);padding:4px;max-height:192px;overflow:hidden}
.lista-tr .rolagem{position:absolute;right:3px;top:6px;width:4px;height:136px;border-radius:999px;background:var(--fbc);opacity:.7}
.lt{display:flex;align-items:center;gap:10px;height:30px;padding:0 12px 0 10px;border-radius:6px}
.lt+.lt{margin-top:1px}
.lt.on{background:var(--selh)}
.lt>label{display:flex;align-items:center;gap:8px;min-width:0;flex:1;color:var(--t2);cursor:pointer}
.lt.on>label{color:var(--t);font-weight:600}
.dest{display:flex;align-items:center;gap:6px;flex:none}
.dest .campo.pq{display:inline-flex;width:auto}
.dest input.campo.pq{width:128px}
.prev-g{border-radius:10px;border:1px solid var(--fb);overflow:hidden}
.prev-g .grade td{height:24px}
.prev-g .grade th{height:28px}
.grade td.tr{background:var(--tint)}
.grade th.tr{background:color-mix(in oklab,var(--acc) 26%,var(--bt2))}
.grade td.semp{color:var(--t3);text-align:center}
.grade td.vazia{color:var(--t3)}
.fundo-dlg{position:relative;border-radius:10px;overflow:hidden;display:flex;justify-content:center;padding:28px 0}
.fundo-dlg>.esq{position:absolute;inset:0;background-color:var(--c);background-image:repeating-linear-gradient(to bottom,transparent 0,transparent 27px,var(--b) 27px,var(--b) 28px)}
.fundo-dlg>.veu{position:absolute;inset:0;background:var(--veu)}
.fundo-dlg>.dlg{position:relative}
"""
cabeca = cabeca.replace("</style>", "\n".join(extra) + "\n" + PROPRIO + "</style>", 1)
defs = achar(r'(<svg width="0" height="0"[^>]*><defs>.*?</defs></svg>)', estados, "Estados")

# --- peças tiradas das pranchetas -----------------------------------------------------------------------------------
renomear = achar(r'<button class="btn sutil lista">(' + UM_SVG + ')Renomear a coluna</button>', csv, "Csv")
lixeira = svg_de(csv, "Excluir", "Csv")
nova_coluna = svg_de(csv, "Nova coluna", "Csv")
doc_csv = achar(r'<a href="#" role="tab" aria-selected="true"><span class="sk s16 arq" aria-hidden="true">(' + UM_SVG + ')<span class="rot">CSV</span>', csv, "Csv")
seta_campo = achar(r'<button class="campo" aria-label="Separador">Ponto e vírgula<span style="flex:1"></span>(' + UM_SVG + ')</button>', csv, "Csv")
mais_cabecalho = achar(r'<th class="nova-col"><button class="ib pq" aria-label="Nova coluna" title="Nova coluna no fim">(' + UM_SVG + ')</button>', csv, "Csv")
fechar = achar(r'<button class="ib" aria-label="Fechar" title="Fechar \(Esc\)">(' + UM_SVG + ')</button>', janelas, "Janelas")
chip_x = achar(r'<span class="chip"><b>ativo</b> igual a <b>1</b><button class="x" aria-label="Tirar o filtro">(' + UM_SVG + ')</button>', dados, "Dados")
condicao = achar(r'<button class="btn sutil pq">(' + UM_SVG + ')Condição</button>', dados, "Dados")
ok_svg = achar(r'<div role="status" class="flut"[^>]*>\s*<span class="sk s16" aria-hidden="true">(' + UM_SVG + ')</span>', json_, "Json")
atencao = achar(r'<div class="aviso" role="alert">\s*<span class="sk s24" aria-hidden="true">(' + UM_SVG + ')</span>', claro, "DadosClaro")
CHECK = '<svg class="ic" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5"></path></svg>'


def pastilha(coluna):
    return achar(r'title="[^"]+">(<span class="sk s16 tipo g\d" aria-hidden="true">' + UM_SVG + r'<span class="rot">[^<]+</span></span>)' + coluna + '</th>', csv, "Csv")


P_DATA, P_TXT, P_INT, P_DEC = pastilha("data"), pastilha("regiao"), pastilha("quantidade"), pastilha("valor")
TIPO = {"data": (P_DATA, "data (detectado)", ""), "txt": (P_TXT, "texto", ""), "int": (P_INT, "número inteiro (detectado)", "num"),
        "dec": (P_DEC, "número decimal (detectado)", "num")}


def estado(titulo, nota, palco, largo=False, estilo_palco=""):
    classe = "estado largo" if largo else "estado"
    st = f' style="{estilo_palco}"' if estilo_palco else ""
    return (f'<section class="{classe}" aria-label="{titulo}"><h2>{titulo}</h2><p class="nota">{nota}</p>'
            f'<div class="palco"{st}>{palco}</div></section>')


def th(nome, tipo, largura=None, classe="", extra=""):
    p, dica, num = TIPO[tipo]
    cls = " ".join(c for c in (num, classe) if c)
    st = f' style="width:{largura}px"' if largura else ""
    return f'<th{f" class=\"{cls}\"" if cls else ""} title="{dica}"{st}{extra}>{p}{nome}</th>'


TH_NUM = '<th class="num" style="color:var(--t3);width:48px;padding:0 8px">#</th>'


def td(valor, classe=""):
    return f'<td{f" class=\"{classe}\"" if classe else ""}>{valor}</td>'


def campo(texto, aria, classe="campo", icone="", estilo_=""):
    st = f' style="{estilo_}"' if estilo_ else ""
    return (f'<button class="{classe}" aria-haspopup="listbox" aria-label="{aria}"{st}>{icone}<span>{texto}</span>'
            f'<span style="flex:1"></span>{seta_campo}</button>')


def mi(texto, atalho="", icone=None, classe="mi", desligado=False):
    ic = icone if icone else '<span class="vz"></span>'
    at = f'<span class="at">{atalho}</span>' if atalho else ""
    dis = ' aria-disabled="true"' if desligado else ""
    return f'<a class="{classe}" role="menuitem" href="#"{dis}>{ic}{texto}{at}</a>'


SEP = '<div class="msep"></div>'
ICONE_CSV16 = f'<span class="sk s16 arq" aria-hidden="true">{doc_csv}<span class="rot">CSV</span></span>'

# --- dados: as vendas do Csv.dc.html (mesma fórmula) e a conferência que tem as mesmas vendas, sem o valor ----------
REGIOES = ["Sul", "Sudeste", "Nordeste", "Norte", "Centro-Oeste"]
VENDEDORES = ["Ana Coutinho", "Bruno Brandão", "Carla Ibiapina", "João Esteves", "Marina Lacerda", "Pedro Guimarães"]
PRODUTOS = ["Caderno universitário", "Caneta gel", "Mochila escolar", "Agenda 2026", "Estojo duplo", "Marca-texto"]
SITUACAO = ["conferida", "conferida", "pendente", "conferida", "divergente"]


def venda(i):
    q = (i * 7) % 23 + 1
    v = f"{q * (12.9 + (i % 6) * 9.35):.2f}".replace(".", ",")
    return {"n": i + 1, "data": f"2025-01-{2 + i:02d}", "regiao": REGIOES[i % 5], "vendedor": VENDEDORES[i % 6],
            "produto": PRODUTOS[(i * 5) % 6], "q": str(q), "v": v, "sit": SITUACAO[(i * 3) % 5]}


# --- (a) o menu do cabeçalho --------------------------------------------------------------------------------------
cab_a = ("<thead><tr>" + TH_NUM + th("data", "data", 96) + th("regiao", "txt", 108) + th("produto", "txt")
         + th("quantidade", "int", 104) + th("valor", "dec", 96, "selcol", ' aria-selected="true"') + "</tr></thead>")
corpo_a = "".join(f'<tr><td class="lin">{r["n"]}</td>{td(r["data"])}{td(r["regiao"])}{td(r["produto"])}{td(r["q"], "num")}{td(r["v"], "num selcol")}</tr>'
                  for r in map(venda, range(9)))
menu_a = ('<div role="menu" aria-label="Coluna valor" class="flut menu-ctx largo" style="right:22px;top:46px">'
          + mi("Copiar coluna", "Ctrl+C") + mi("Colar como nova coluna", "Ctrl+V") + mi("Colar sobre esta coluna")
          + mi("Colar com correspondência", "Ctrl+Shift+V") + mi("Trazer colunas de outro arquivo", icone=TRAZER)
          + SEP + mi("Renomear a coluna", "F2", renomear) + mi("Nova coluna depois desta", icone=nova_coluna)
          + SEP + mi("Excluir a coluna", icone=lixeira, classe="mi perigo") + "</div>")
cena_a = estado(
    "Menu do cabeçalho: copiar e colar",
    "O clique no cabeçalho seleciona a coluna inteira (Ctrl e Shift somam colunas); o clique direito ou Shift+F10 abre o "
    "menu. Copiar leva o texto com tabulações e o cabeçalho na primeira linha, que cola direto no Excel; com várias "
    "selecionadas, o item vira \"Copiar 3 colunas\". Colar como nova coluna entra à direita desta; Colar sobre esta coluna "
    "substitui os valores; Colar com correspondência abre o Trazer colunas com a área de transferência como origem.",
    '<div class="doc"><table class="grade">' + cab_a + "<tbody>" + corpo_a + "</tbody></table>"
    '<div class="rodape"><span>1.204 linhas · 6 colunas</span><span style="color:var(--t3)">· coluna valor selecionada</span></div></div>'
    + menu_a,
    estilo_palco="min-height:370px")

# --- (b) colar com filtro ligado ------------------------------------------------------------------------------------
faixa_b = ('<div class="faixa"><span class="tit">Filtros</span>'
           f'<span class="chip"><b>regiao</b> igual a <b>Sul</b><button class="x" aria-label="Tirar o filtro">{chip_x}</button></span>'
           f'<button class="btn sutil pq">{condicao}Condição</button><span style="flex:1"></span>'
           '<span style="color:var(--t3)">236 de 1.180 linhas</span><button class="btn sutil pq">Limpar filtros</button></div>')
cab_b = ("<thead><tr>" + TH_NUM + th("data", "data", 96) + th("regiao", "txt", 108, "selcol", ' aria-selected="true"')
         + th("vendedor", "txt") + th("situacao", "txt", 112) + "</tr></thead>")
corpo_b = "".join(f'<tr><td class="lin">{r["n"]}</td>{td(r["data"])}{td(r["regiao"], "selcol")}{td(r["vendedor"])}{td(r["sit"])}</tr>'
                  for r in map(venda, range(0, 40, 5)))
DICA = "Tire o filtro e a ordenação para colar por posição, ou use Colar com correspondência."
menu_b = ('<div role="menu" aria-label="Coluna regiao" class="flut menu-ctx largo" style="left:118px;top:88px;width:290px">'
          + mi("Copiar coluna", "Ctrl+C")
          + mi("Colar como nova coluna", "Ctrl+V", classe="mi off", desligado=True)
          + mi("Colar sobre esta coluna", classe="mi off", desligado=True)
          + mi("Colar com correspondência", "Ctrl+Shift+V") + mi("Trazer colunas de outro arquivo", icone=TRAZER)
          + SEP + mi("Renomear a coluna", "F2", renomear) + mi("Nova coluna depois desta", icone=nova_coluna)
          + SEP + mi("Excluir a coluna", icone=lixeira, classe="mi perigo") + "</div>"
          + f'<div role="tooltip" class="flut dica-col" style="left:414px;top:126px">{DICA}</div>')
cena_b = estado(
    "Colar com filtro ou ordenação",
    "A cola por posição põe a linha 1 colada na linha 1 do arquivo. Com filtro ou ordenação ligados, a ordem da tela não "
    "é a do arquivo (as linhas 1, 6, 11…), então os dois Colar ficam desligados e a dica diz o caminho. Colar com "
    "correspondência continua ligado: ele casa as linhas por uma coluna, sem depender da ordem.",
    '<div class="doc">' + faixa_b + '<table class="grade">' + cab_b + "<tbody>" + corpo_b + "</tbody></table>"
    '<div class="rodape"><span>Linhas 1–236 de 236</span><span style="color:var(--t3)">· filtrado de 1.180</span></div></div>'
    + menu_b,
    estilo_palco="min-height:370px")

# --- (c) quantidades diferentes -------------------------------------------------------------------------------------
aviso_c = ('<div class="no-doc"><div class="aviso-j" role="alert">'
           f'<span class="sk s24" aria-hidden="true">{atencao}</span>'
           '<div class="corpo"><span>A coluna colada tem 1.204 valores e o arquivo, 1.180 linhas.</span>'
           '<div class="acoes"><button class="btn pri pq">Colar 1.180</button><button class="btn pq">Acrescentar 24 linhas</button>'
           '<button class="btn sutil pq" title="Esc">Cancelar</button></div></div></div></div>')
cab_c1 = ("<thead><tr>" + TH_NUM + th("data", "data", 96) + th("regiao", "txt", 108, "selcol", ' aria-selected="true"')
          + th("vendedor", "txt") + th("situacao", "txt", 104) + "</tr></thead>")
corpo_c1 = "".join(f'<tr><td class="lin">{r["n"]}</td>{td(r["data"])}{td(r["regiao"], "selcol")}{td(r["vendedor"])}{td(r["sit"])}</tr>'
                   for r in map(venda, range(7)))
cab_c2 = ("<thead><tr>" + TH_NUM + th("data", "data", 96) + th("regiao", "txt", 100)
          + '<th class="nova num" style="width:104px"><span class="th-edit" role="textbox" aria-label="Nome da coluna colada" tabindex="0">'
            '<span class="sel-txt">valor</span></span></th>'
          + th("vendedor", "txt") + th("situacao", "txt", 96) + "</tr></thead>")
corpo_c2 = "".join(f'<tr><td class="lin">{r["n"]}</td>{td(r["data"])}{td(r["regiao"])}{td(r["v"], "num nova")}{td(r["vendedor"])}{td(r["sit"])}</tr>'
                   for r in map(venda, range(10)))
toast_c = (f'<div role="status" class="flut toast" style="position:absolute;left:50%;bottom:48px;transform:translateX(-50%)">'
           f'<span class="sk s16" aria-hidden="true">{ok_svg}</span>Coluna valor colada · 1.180 linhas</div>')
cena_c = estado(
    "Quantidades diferentes e a coluna colada",
    "Quando a coluna colada não tem o mesmo número de linhas do arquivo, um aviso compacto no topo do documento pergunta, "
    "sem janela. Com menos valores, as linhas que sobram ficam vazias e o aviso só informa (\"24 linhas ficaram vazias\"); "
    "Colar sobre esta coluna com valores no caminho pergunta no mesmo aviso (\"Substituir 1.180 valores de regiao?\"). "
    "Depois de colar, o cabeçalho abre em edição com o nome sugerido selecionado (o da primeira linha, ou coluna_N): "
    "Enter confirma, Esc mantém. A cola inteira é uma alteração só no desfazer.",
    '<div class="etapas">'
    '<div class="etapa"><span class="tit">1 · Ao colar depois de regiao</span><div class="doc">' + aviso_c
    + '<table class="grade">' + cab_c1 + "<tbody>" + corpo_c1 + "</tbody></table>"
    '<div class="rodape"><span>1.180 linhas · 5 colunas</span></div></div></div>'
    '<div class="etapa"><span class="tit">2 · Depois de Colar 1.180</span><div class="doc" style="position:relative;flex:1">'
    '<table class="grade">' + cab_c2 + "<tbody>" + corpo_c2 + "</tbody></table><div style=\"flex:1\"></div>"
    '<div class="rodape"><span>1.180 linhas · 6 colunas</span><span style="color:var(--acct)">· 1 alteração não salva</span></div>'
    + toast_c + "</div></div></div>",
    largo=True)

# --- (d) a janela Trazer colunas ------------------------------------------------------------------------------------
ESTOQUE = [  # codigo, produto, deposito, estoque, regiao antes, regiao depois, preco, categoria (None: sem par)
    ("004718", "Caderno universitário", "Guarulhos", "412", "Sudeste", "Sudeste", "24,90", "Cadernos"),
    ("004725", "Caneta gel", "Guarulhos", "1.380", "", "Sul", "4,50", "Escrita"),
    ("004731", "Mochila escolar", "Contagem", "96", "Sudeste", "Sudeste", "129,90", "Mochilas"),
    ("019044", "Agenda 2025", "Contagem", "18", "Norte", "Norte", None, None),
    ("005102", "Estojo duplo", "Guarulhos", "233", "", "Centro-Oeste", "27,40", "Estojos"),
    ("005117", "Marca-texto", "Recife", "2.045", "Nordeste", "Nordeste", "6,90", "Escrita"),
    ("005123", "Agenda 2026", "Recife", "640", "Nordeste", "Nordeste", "39,90", "Agendas"),
    ("005130", "Caderno brochura", "Guarulhos", "1.112", "", "Sudeste", "12,90", "Cadernos"),
    ("005148", "Lápis HB", "Contagem", "3.400", "Sudeste", "Sudeste", "1,20", "Escrita"),
    ("005155", "Borracha branca", "Recife", "2.210", "Nordeste", "Nordeste", "1,80", "Escrita"),
]
ORIGEM = [  # coluna, tipo, marcada, destino
    ("produto", P_TXT, False, None), ("categoria", P_TXT, True, ("nova", "categoria")), ("preco", P_DEC, True, ("nova", "preco")),
    ("regiao", P_TXT, True, ("atualizar", "regiao")), ("fornecedor", P_TXT, False, None), ("unidade", P_TXT, False, None),
    ("peso_kg", P_DEC, False, None), ("atualizado_em", P_DATA, False, None),
]


def linha_origem(nome, p, marcada, destino):
    ck = ' checked=""' if marcada else ""
    rotulo = f'<label><input type="checkbox"{ck} aria-label="Trazer {nome}">{p}{nome}</label>'
    if not destino:
        return f'<div class="lt">{rotulo}</div>'
    modo, alvo = destino
    if modo == "nova":
        d = (campo("nova coluna", f"Destino de {nome}", "campo pq")
             + f'<input class="campo pq" value="{alvo}" aria-label="Nome da coluna nova para {nome}">')
    else:
        d = (campo("atualizar", f"Destino de {nome}", "campo pq") + campo(alvo, f"Coluna a atualizar com {nome}", "campo pq")
             + campo("só as células vazias", f"Como atualizar {alvo}", "campo pq"))
    return f'<div class="lt on">{rotulo}<div class="dest">{d}</div></div>'


# a lista mostra 6 linhas e rola (peso_kg e atualizado_em ficam embaixo); o trilho de rolagem é desenhado, porque a
# captura sem janela esconde as barras
lista_tr = ('<div class="lista-tr" role="group" aria-label="Colunas de catalogo_2025.csv">' + "".join(linha_origem(*o) for o in ORIGEM)
            + '<span class="rolagem" aria-hidden="true"></span></div>')
cab_prev = ('<thead><tr><th class="num" style="color:var(--t3);width:40px;padding:0 8px">#</th>'
            + th("codigo", "txt", 78) + th("produto", "txt") + th("regiao", "txt", 104, "tr") + th("preco", "dec", 84, "tr")
            + th("categoria", "txt", 104, "tr") + "</tr></thead>")


def linha_prev(n, e):
    codigo, produto, _dep, _est, antes, depois, preco, cat = e
    if preco is None:
        return (f'<tr><td class="lin">{n}</td><td class="chave">{codigo}</td>{td(produto)}{td(depois)}'
                '<td class="semp">—</td><td class="semp">—</td></tr>')
    reg = td(depois, "tr") if antes == "" else td(depois)
    return f'<tr><td class="lin">{n}</td><td class="chave">{codigo}</td>{td(produto)}{reg}{td(preco, "num tr")}{td(cat, "tr")}</tr>'


prev = ('<div class="prev-g"><table class="grade">' + cab_prev + "<tbody>"
        + "".join(linha_prev(n, e) for n, e in enumerate(ESTOQUE[:6], 1)) + "</tbody></table></div>")
icone20 = (f'<span class="sk s20 arq" aria-hidden="true">{lado(doc_csv, 20)}'
           '<span class="rot" style="font-size:6.8px;letter-spacing:0">CSV</span></span>')
opcao = lambda txt, on: (f'<button class="tg" aria-pressed="{"true" if on else "false"}">{CHECK if on else ""}{txt}</button>')
dialogo = (
    '<section class="dlg flut tc" role="dialog" aria-label="Trazer colunas para estoque_2026.csv">'
    f'<header>{icone20}<h2 style="margin-left:10px">Trazer colunas</h2><span style="color:var(--t3);margin-left:8px">para estoque_2026.csv</span>'
    f'<span style="flex:1"></span><button class="ib" aria-label="Fechar" title="Fechar (Esc)">{fechar}</button></header>'
    '<div class="corpo-tc">'
    # 1. de onde
    '<div class="bloco"><span class="tit" id="tc-de">De onde</span>'
    '<div class="casar">' + campo("catalogo_2025.csv", "De onde", icone=ICONE_CSV16)
    + '<span></span><span class="meta">5.038 linhas · 9 colunas</span></div></div>'
    # 2. casar as linhas
    '<div class="bloco"><span class="tit">Casar as linhas onde</span>'
    '<div class="casar">' + campo("codigo", "Coluna de estoque_2026.csv") + '<span class="igual" aria-hidden="true">=</span>'
    + campo("cod_produto", "Coluna de catalogo_2025.csv")
    + '<span class="meta">deste arquivo</span><span></span><span class="meta">de catalogo_2025.csv</span></div>'
    '<div class="medida"><div class="txt" aria-live="polite"><strong>4.812 de 5.000 linhas têm par</strong><span>(96%)</span></div>'
    '<div class="medidor" role="meter" aria-label="Linhas com par" aria-valuemin="0" aria-valuemax="5000" aria-valuenow="4812">'
    '<div style="width:96.24%"></div></div></div>'
    '<div class="opcoes" role="group" aria-label="Como comparar as chaves">'
    + opcao("Ignorar maiúsculas", True) + opcao("Ignorar espaços nas pontas", True) + opcao("Números pelo valor", False) + "</div>"
    '<div class="aviso-j" role="status" style="margin-top:4px">'
    f'<span class="sk s24" aria-hidden="true">{atencao}</span>'
    '<div class="corpo"><span>38 códigos aparecem mais de uma vez em catalogo_2025.csv.</span>'
    '<div class="acoes"><span class="rot-sel" id="tc-usar">Usar:</span>'
    + campo("a primeira", "Qual usar das repetidas", "campo pq") + '<a class="link" href="#" style="margin-left:6px">Ver quais</a></div></div></div>'
    "</div>"
    # 3. trazer
    '<div class="bloco"><div class="cab-bloco"><span class="tit">Trazer</span><span class="meta">3 de 8 colunas</span>'
    '<span style="flex:1"></span><button class="btn sutil pq">Marcar todas</button></div>' + lista_tr + "</div>"
    # prévia
    '<div class="bloco"><div class="cab-bloco"><span class="tit">Prévia</span><span class="meta">as 6 primeiras linhas de estoque_2026.csv</span></div>'
    + prev + "</div>"
    "</div>"
    '<footer><span style="flex:1"></span><button class="btn sutil" title="Esc">Cancelar</button>'
    '<button class="btn pri">Trazer 3 colunas</button></footer></section>')
cena_d = estado(
    "Trazer colunas",
    "Uma janela de 640 px que se lê como uma frase: de catalogo_2025.csv, casar codigo com cod_produto, trazer categoria, "
    "preco e regiao. A origem pode ser qualquer documento aberto com tabela, a área de transferência ou outro arquivo. "
    "O medidor recalcula a cada troca de coluna ou de opção; \"007\" e \"7\" só casam com Números pelo valor, porque "
    "códigos como CEP e CPF guardam os zeros. Cada coluna marcada diz o destino: uma coluna nova (com o nome editável; "
    "se já existir, sugere \"preco (catalogo)\") ou atualizar uma que existe, substituindo ou só nas células vazias. Na "
    "prévia, o tingido é o que vem da origem e o \"—\" é a linha sem par. Trazer desliga com nenhuma marcada ou 0% de par.",
    '<div class="fundo-dlg"><div class="esq" aria-hidden="true"></div><div class="veu" aria-hidden="true"></div>' + dialogo + "</div>",
    largo=True, estilo_palco="padding:0")

# --- (e) depois de trazer -------------------------------------------------------------------------------------------
desfazer = botao(csv, "Desfazer", "Csv").replace('title="Desfazer (Ctrl+Z)"', 'title="Desfazer Trazer 3 colunas (Ctrl+Z)"')
barra_e = ('<div role="toolbar" aria-label="Ferramentas (trecho)" class="vidro barra-mini">'
           + botao(csv, "Nova linha", "Csv") + botao(csv, "Nova coluna", "Csv")
           + f'<button class="ib" aria-label="Trazer colunas" title="Trazer colunas">{TRAZER}</button>'
           + botao(csv, "Duplicar", "Csv") + botao(csv, "Excluir", "Csv")
           + f'<span class="com-dica">{desfazer}<span class="dica-fixa flut" role="tooltip">Desfazer Trazer 3 colunas</span></span>'
           + botao(csv, "Refazer", "Csv") + "</div>")
cab_e = ("<thead><tr>" + TH_NUM + th("codigo", "txt", 96) + th("produto", "txt") + th("deposito", "txt", 140)
         + th("estoque", "int", 110) + th("regiao", "txt", 140) + th("preco", "dec", 110, "nova") + th("categoria", "txt", 140, "nova")
         + f'<th class="nova-col" style="width:34px"><button class="ib pq" aria-label="Nova coluna" title="Nova coluna no fim">{mais_cabecalho}</button></th>'
         + "</tr></thead>")
corpo_e = "".join(
    f'<tr><td class="lin">{n}</td>{td(c)}{td(p)}{td(d)}{td(s, "num")}{td(depois)}{td(pr or "", "num nova")}{td(ca or "", "nova")}<td></td></tr>'
    for n, (c, p, d, s, _a, depois, pr, ca) in enumerate(ESTOQUE, 1))
toast_e = ('<div role="status" class="flut toast com-acao" style="position:absolute;left:50%;bottom:50px;transform:translateX(-50%)">'
           f'<span class="sk s16" aria-hidden="true">{ok_svg}</span>3 colunas trazidas · 188 linhas sem par'
           '<button class="btn pq">Marcar as linhas sem par</button></div>')
cena_e = estado(
    "Depois de trazer",
    "As colunas novas entram no fim, e a regiao ganhou valor só onde estava vazia (linhas 2, 5 e 8). Tudo é uma alteração só "
    "no desfazer (a dica diz qual) e conta como não salva. O aviso flutuante dá o resultado e a próxima ação: Marcar as "
    "linhas sem par usa a marcação de linhas, e daí Excluir, Copiar ou filtrar ficam a um clique. A barra do CSV ganha o "
    "botão Trazer colunas (silhueta nova trazer_colunas) logo depois de Nova coluna; a seção Coluna das Propriedades "
    "também tem a ação.",
    barra_e + '<div class="doc" style="margin-top:26px;position:relative"><table class="grade">' + cab_e + "<tbody>" + corpo_e
    + "</tbody></table>"
    '<div class="rodape"><span>5.000 linhas · 7 colunas</span><span style="color:var(--acct)">· 1 alteração não salva</span></div>'
    + toast_e + "</div>",
    largo=True)

quadro = '<div class="quadro">' + cena_a + cena_b + cena_c + cena_d + cena_e + "</div>"
corpo = (f'<div class="est {{{{classes}}}}" style="width:100%;min-height:{ALTURA}px;position:relative;background:var(--ambiente)">\n'
         + defs + "\n" + quadro + "\n</div>")
script = estados[estados.index('<script type="text/x-dc"'):]
if not re.search(r'"\$preview":\{"width":1440,"height":\d+\}', script):
    raise SystemExit("colunas: o script do Estados.dc.html não tem o $preview esperado")
script = re.sub(r'"\$preview":\{"width":1440,"height":\d+\}', f'"$preview":{{"width":1440,"height":{ALTURA}}}', script)
pagina = cabeca + "</helmet>\n" + corpo + "\n</x-dc>\n" + script

destino = PASTA / "Colunas.dc.html"
if destino.is_file() and MARCA not in destino.read_text(encoding="utf-8"):
    raise SystemExit("colunas: Colunas.dc.html existe sem a marca deste gerador; não gravei nada")

# o lugar no canvas: x 0, abaixo da prancheta mais baixa da coluna 0, sem cobrir nenhuma outra
lay = PASTA / "canvas.json"
j = json.loads(lay.read_text(encoding="utf-8"))
entrada = {"expand": "fill", "h": ALTURA, "title": TITULO, "w": LUGAR["w"], "x": LUGAR["x"], "y": LUGAR["y"]}
for nome, b in j["boards"].items():
    if nome == "Colunas.dc.html":
        continue
    if b["x"] < LUGAR["x"] + LUGAR["w"] and LUGAR["x"] < b["x"] + b["w"] and b["y"] < LUGAR["y"] + ALTURA and LUGAR["y"] < b["y"] + b["h"]:
        raise SystemExit(f"colunas: o lugar da prancheta 16 cobre {nome}; não gravei nada")
    if b["x"] == 0 and b["y"] + b["h"] + 120 > LUGAR["y"]:
        raise SystemExit(f"colunas: {nome} desce até {b['y'] + b['h']}; a prancheta 16 precisa de 120 px de folga")

for caminho, texto in ((destino, pagina), (PASTA / "Icones.dc.html", icones_novo)):
    if not caminho.is_file() or caminho.read_text(encoding="utf-8") != texto:
        caminho.write_text(texto, encoding="utf-8", newline="\n")
        print("gravado:", caminho.name, len(texto), "bytes")
    else:
        print("igual:", caminho.name)
mudou = j["boards"].get("Colunas.dc.html") != entrada or "Colunas.dc.html" not in j["order"]
j["boards"]["Colunas.dc.html"] = entrada
if "Colunas.dc.html" not in j["order"]:
    j["order"].append("Colunas.dc.html")
if mudou:
    lay.write_text(json.dumps(j, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(f"canvas.json: prancheta 16 em x {LUGAR['x']}, y {LUGAR['y']}, {LUGAR['w']} × {ALTURA}")
else:
    print("igual: canvas.json")
