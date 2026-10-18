"""Prancheta 15: estados a implementar (escuro), da revisão crítica de 15/10/2026 aprovada pelo Isaías em 16/10/2026
(da revisão de 15/10/2026 e a D9):
- N3 e D9: a consulta executando, com o Executar ocupado (o arco do Exportar girando, dica "Executando") e o
  Interromper (botão de risco) ao lado; rodapé "Executando · 1,4 s";
- N1: o JSON inválido (ícone de erro no estado, linha do erro marcada, Formatar desligado, a última estrutura válida
  em --t3 e o aviso de que salvou com erro);
- N2: fechar a aba alterada (o × no lugar do ponto, no hover e no foco, e a pergunta Salvar, Não salvar, Cancelar);
- N4: a coluna nova no CSV (inserida depois da coluna da célula ativa, com o cabeçalho em edição);
- N5: o Histórico vazio;
- D9: linhas marcadas no CSV, o menu da coluna no CSV (renomear, excluir), Copiar caminho no Explorador e a ficha
  da linha só leitura (banco aberto só para leitura).
Usa a folha de estilo e as definições do Dados.dc.html, mais as regras que faltam tiradas da Consulta, do CSV, do
JSON, do Exportar e das Janelas, como o janelas.py faz com o Exportar. Os ícones e os botões vêm das próprias
pranchetas, pelo aria-label ou pelo texto. Rode depois dos geradores revisao_*.py, do janelas.py e do preferencias.py.
Gera ../canvas/Estados.dc.html e refaz a prancheta inteira (1440 px de largura).
Uso: python estados_novos.py [pasta das pranchetas]"""
import json
import re
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
PASTA = Path(sys.argv[1]) if len(sys.argv) > 1 else AQUI.parent / "canvas"
sys.stdout.reconfigure(encoding="utf-8")
UM_SVG = r'<svg\b[^>]*>(?:(?!</svg>).)*</svg>'


def ler(nome):
    return (PASTA / f"{nome}.dc.html").read_text(encoding="utf-8")


dados, consulta, csv, json_, exportar, janelas = (ler(n) for n in ("Dados", "Consulta", "Csv", "Json", "Exportar", "Janelas"))
inicio, claro, icones = ler("Inicio"), ler("DadosClaro"), ler("Icones")


def achar(padrao, texto, onde, grupo=1):
    m = re.search(padrao, texto, re.S)
    if not m:
        raise SystemExit(f"{onde}: não achei {padrao[:90]!r}")
    return m.group(grupo)


def botao(texto, aria, onde):
    """O botão inteiro da barra pelo aria-label (o primeiro)."""
    return achar(r'(<button class="ib[^"]*"[^>]*aria-label="' + re.escape(aria) + r'"[^>]*>' + UM_SVG + '</button>)', texto, onde)


def svg_de(texto, aria, onde):
    return achar(r'<button class="ib[^"]*"[^>]*aria-label="' + re.escape(aria) + r'"[^>]*>(' + UM_SVG + ')</button>', texto, onde)


def lado(svg, n):
    return re.sub(r'width="[^"]+" height="[^"]+"', f'width="{n}" height="{n}"', svg, count=1)


def estilo(texto):
    return texto[texto.index("<style>") + len("<style>"):texto.index("</style>")].strip("\n").split("\n")


# --- folha: a do Dados + o que falta das outras -------------------------------------------------------------------
cabeca = dados[: dados.index("</helmet>")]
cabeca = re.sub(r"<title>.*?</title>", "<title>Estados a implementar</title>", cabeca, count=1)
ja = set(estilo(dados))
PREFIXOS = {
    "Main": (".mi", ".msep"),
    "Consulta": (".hist", ".cod", ".kw", ".erro", ".seg .n", ".seg .erro-t"),
    "Csv": (".grade td.alt", ".grade th.nova-col", ".grade td.lin{padding"),
    "Json": (".migalha", ".estado-v", ".dica-v"),
    "Exportar": (".dlg", ".blk"),
    "Janelas": (".aviso-j", ".ficha", ".fl", ".nulo-f", ".dlg.larga", ".sk.s24.tipo", "input.campo", ".campo::placeholder",
                ".campo.nulo", ".campo.so-leitura", ".est.claro .campo.so-leitura"),
}
extra = []
for nome, prefixos in PREFIXOS.items():
    for linha in estilo({"Main": ler("Main"), "Consulta": consulta, "Csv": csv, "Json": json_, "Exportar": exportar, "Janelas": janelas}[nome]):
        if linha.startswith(prefixos) and linha not in ja and linha not in extra:
            extra.append(linha)
PROPRIO = """/* prancheta 15: estados a implementar */
.quadro{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:28px 24px;padding:28px}
.estado{display:flex;flex-direction:column;gap:10px;min-width:0}
.estado.largo{grid-column:span 2}
.estado h2{margin:0;font-size:14px;font-weight:600;color:var(--t)}
.estado .nota{margin:0;color:var(--t3);font-size:11.5px;line-height:17px;max-width:900px}
.palco{position:relative;border-radius:10px;border:1px solid var(--vidrob);background:var(--fundo0);padding:14px;display:flex;flex-direction:column;gap:10px;min-width:0;overflow:hidden}
.barra-mini{display:flex;align-items:center;gap:2px;height:44px;padding:0 8px;border-radius:10px;align-self:flex-start}
.ib.risco,.ib.risco:hover{color:#fff;border-color:#7f2219;background:var(--textura),var(--volrisco)}
.ib.risco{box-shadow:inset 0 1px 0 rgba(255,255,255,.32),0 2px 8px rgba(192,57,43,.35)}
.ib.risco:hover{box-shadow:inset 0 1px 0 rgba(255,255,255,.32),0 0 0 1px rgba(232,96,80,.6),0 0 20px rgba(232,96,80,.55)}
.ib.ocupado{cursor:progress;filter:saturate(.75);opacity:.8}
.ib .gira{width:16px;height:16px;stroke-width:2;animation:gira 1s linear infinite}
@keyframes gira{to{transform:rotate(360deg)}}
.dica-fixa{position:absolute;top:calc(100% + 6px);left:50%;transform:translateX(-50%);padding:5px 10px;border-radius:8px;white-space:nowrap;font-size:11px;font-weight:400;color:var(--t);background:var(--flut);border:1px solid var(--vidrob);box-shadow:var(--sh);z-index:40}
.dica-fixa.esq{left:-4px;transform:none}
.com-dica{position:relative;display:inline-flex}
.estado-v .dica-v.fixa{opacity:1;transform:none}
.aba .mod+.x{display:none}
.aba:hover .mod,.aba.foco .mod{display:none}
.aba:hover .mod+.x,.aba.foco .mod+.x{display:inline-flex}
.aba.foco .x{background:var(--h);color:var(--t)}
.cod.erro-l{background:color-mix(in oklab,var(--perigo) 9%,transparent)}
.margem .erro-n{color:var(--perigo);font-weight:600}
.velha .no{color:var(--t3)}
.velha .sk{opacity:.6}
.th-edit{display:flex;align-items:center;height:22px;padding:0 6px;border-radius:4px;border:1px solid var(--acc);background:var(--f);box-shadow:0 0 0 1px var(--acc),0 0 10px var(--brilho);color:var(--t);font-weight:600;min-width:0}
.th-edit .sel-txt{background:color-mix(in oklab,var(--acc) 55%,transparent);color:#fff;border-radius:2px;padding:0 1px}
.grade th.nova{padding:0 4px;background:var(--tint)}
.grade td.nova{background:color-mix(in oklab,var(--acc) 8%,transparent)}
.menu-ctx{position:absolute;width:250px;padding:5px;border-radius:12px}
.mi .sil{width:14px;height:14px;color:var(--t2)}
.mi.perigo,.mi.perigo .sil{color:var(--perigo)}
.vazio{flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:12px;padding:24px;text-align:center}
.nota-j{display:flex;align-items:center;gap:12px;padding:9px 12px;border-radius:10px;width:fit-content;max-width:min(100%,560px);margin-inline:auto;text-wrap:pretty;background:var(--h);border:1px solid var(--vidrob);color:var(--t2)}
.toast{display:flex;align-items:center;gap:8px;padding:9px 16px;border-radius:999px;white-space:nowrap;align-self:center}
.estado input[type=checkbox]:disabled{opacity:.5;cursor:default;filter:none}
"""
cabeca = cabeca.replace("</style>", "\n".join(extra) + "\n" + PROPRIO + "</style>", 1)
defs = achar(r'(<svg width="0" height="0"[^>]*><defs>.*?</defs></svg>)', dados, "Dados")

# --- peças tiradas das pranchetas -----------------------------------------------------------------------------------
ARCO = achar(r'<button class="btn pri" disabled="">(<svg class="ic" viewBox="0 0 24 24"><path d="M12 3\.5a8\.5 8\.5 0 1 1-8\.5 8\.5"></path></svg>)Gerando arquivo</button>',
             exportar, "Exportar")
ARCO = ARCO.replace('<svg class="ic"', '<svg class="ic gira"', 1)
# Interromper: o quadrado de parar, na silhueta (cheio, cantos de 2)
PARAR = '<svg class="sil" viewBox="0 0 24 24"><path d="M7.5 5.5h9a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-9a2 2 0 0 1-2-2v-9a2 2 0 0 1 2-2z"></path></svg>'
erro_svg = achar(r'<div class="erro">\s*<span class="sk s24" aria-hidden="true">(' + UM_SVG + ')</span>', consulta, "Consulta")
atencao = achar(r'<div class="aviso" role="alert">\s*<span class="sk s24" aria-hidden="true">(' + UM_SVG + ')</span>', claro, "DadosClaro")
mostrar_na_pasta = achar(r'<button class="ib pq" aria-label="Mostrar na pasta" title="Mostrar na pasta">(' + UM_SVG + ')</button>', inicio, "Inicio")
copiar_caminho = achar(r'<button class="btn sutil lista">(' + UM_SVG + ')Copiar caminho</button>', csv, "Csv")
renomear = achar(r'<button class="btn sutil lista">(' + UM_SVG + ')Renomear a coluna</button>', csv, "Csv")
lixeira = svg_de(csv, "Excluir", "Csv")
nova_coluna = svg_de(csv, "Nova coluna", "Csv")
fechar_x = achar(r'<button class="x" aria-label="Fechar aba">(' + UM_SVG + ')</button>', csv, "Csv")
doc_csv = achar(r'<a href="#" role="tab" aria-selected="true"><span class="sk s16 arq" aria-hidden="true">(' + UM_SVG + ')<span class="rot">CSV</span>', csv, "Csv")
doc_json = achar(r'<span class="migalha"><span class="sk s16 arq l4" aria-hidden="true">(' + UM_SVG + ')<span class="rot">JSON</span>', json_, "Json")
seta_migalha = achar(r'<span style="margin-left:3px">config\.json</span>(' + UM_SVG + ')conexoes', json_, "Json")


def pastilha_csv(coluna):
    return achar(r'title="[^"]+">(<span class="sk s16 tipo g\d" aria-hidden="true">' + UM_SVG + r'<span class="rot">[^<]+</span></span>)' + coluna + '</th>', csv, "Csv")


def cadeado(n):
    """O Bloqueado da biblioteca (prancheta 9) num tamanho fixo de até 24 px, com o contorno firme do item 13."""
    svg = achar(r'<sc-if value="\{\{z\.st_bloqueado\}\}"[^>]*>(' + UM_SVG + ')</sc-if>', icones, "Icones")
    svg = svg.replace('width="{{z.s}}" height="{{z.s}}"', f'width="{n}" height="{n}"')
    svg = svg.replace('<rect class="cont" filter="url(#icSombra)"', '<rect filter="url(#icSombra)"')
    return re.sub(r'(<rect filter="url\(#icSombra\)"[^>]*style=")stroke-opacity:\.35"', r'\1stroke-opacity:.35;stroke-width:2.4"', svg, count=1)


def estado(titulo, nota, palco, largo=False, estilo_palco=""):
    classe = "estado largo" if largo else "estado"
    st = f' style="{estilo_palco}"' if estilo_palco else ""
    return (f'<section class="{classe}" aria-label="{titulo}"><h2>{titulo}</h2><p class="nota">{nota}</p>'
            f'<div class="palco"{st}>{palco}</div></section>')


def barra(*botoes):
    return '<div role="toolbar" aria-label="Ferramentas (trecho)" class="vidro barra-mini">' + "".join(botoes) + "</div>"


SEP = '<span class="sep"></span>'

# --- N3 + D9: consulta executando --------------------------------------------------------------------------------
executar_ocupado = (f'<span class="com-dica"><button class="ib pri ocupado" aria-disabled="true" aria-label="Executar" title="Executando">'
                    f'{ARCO}</button><span class="dica-fixa" role="tooltip">Executando</span></span>')
interromper = f'<button class="ib risco" aria-label="Interromper" title="Interromper a consulta (Esc)">{PARAR}</button>'
faixa_banco = achar(r'(<div class="faixa"><span class="sk s16" aria-hidden="true">' + UM_SVG + r'</span><span style="color:var\(--t2\)">exemplo\.db</span>)',
                    consulta, "Consulta")
editor_n3 = (
    '<div style="height:120px;display:flex;background:var(--c)">'
    '<div class="cod" aria-hidden="true" style="width:46px;flex:none;text-align:right;padding:10px 12px 0 0;color:var(--t3);border-right:1px solid var(--b);background:var(--p)">1\n2\n3</div>'
    '<div style="flex:1;min-width:0;padding:10px 0 0;overflow:hidden">'
    '<div class="cod" style="padding:0 14px"><span class="kw">SELECT</span> e<span class="pu">.</span>tipo<span class="pu">,</span> count<span class="pu">(*)</span> '
    '<span class="kw">AS</span> eventos<span class="pu">,</span> sum<span class="pu">(</span>p<span class="pu">.</span>total<span class="pu">)</span> <span class="kw">AS</span> total</div>'
    '<div class="cod" style="padding:0 14px">  <span class="kw">FROM</span> eventos e <span class="kw">JOIN</span> pedidos p <span class="kw">ON</span> p<span class="pu">.</span>cliente_id '
    '<span class="pu">=</span> e<span class="pu">.</span>cliente_id</div>'
    '<div class="cod" style="padding:0 14px"> <span class="kw">GROUP BY</span> e<span class="pu">.</span>tipo <span class="kw">ORDER BY</span> <span class="nu">2</span> <span class="kw">DESC</span><span class="pu">;</span></div>'
    '</div></div>')
n3 = estado(
    "Consulta executando",
    "Executar fica desligado, com o arco girando no lugar do triângulo e a dica \"Executando\"; Ctrl+Enter é ignorado. "
    "Ao lado aparece Interromper (sqlite3_interrupt, botão de risco, Esc). O rodapé conta o tempo ao vivo, repintando a "
    "cada ~250 ms só enquanto executa. No kit, o botão redondo só de ícone ganha o estado ocupado.",
    barra(botao(consulta, "Desfazer", "Consulta"), botao(consulta, "Refazer", "Consulta"), SEP,
          botao(consulta, "Filtrar", "Consulta"), botao(consulta, "Atualizar", "Consulta"), SEP,
          executar_ocupado, interromper, botao(consulta, "Nova consulta", "Consulta"), botao(consulta, "Exportar", "Consulta"))
    + '<div class="doc" style="margin-top:22px">' + faixa_banco + '<span style="flex:1"></span><span style="color:var(--t3)">Linha 3, coluna 50</span></div>'
    + editor_n3
    + '<div class="rodape" aria-live="polite"><span>Executando · 1,4 s</span></div></div>',
    largo=True)

# --- N1: JSON inválido ---------------------------------------------------------------------------------------------
LINHAS_JSON = [
    (9, '      <span class="ch">"tempo_limite"</span><span class="pu">:</span> <span class="nu">30</span><span class="pu">,</span>'),
    (10, '      <span class="ch">"somente_leitura"</span><span class="pu">:</span> <span class="kw">false</span>'),
    (11, '    <span class="pu">},</span>'),
    (12, '    <span class="pu">{</span>'),
    (13, '      <span class="ch">"nome"</span><span class="pu">:</span> <span class="st">"Relatórios"</span>'),
    (14, '      <span class="ch" style="text-decoration:underline wavy var(--perigo);text-underline-offset:3px">"arquivo"</span>'
         '<span class="pu">:</span> <span class="st">"D:\\\\relatorios\\\\financeiro.sqlite"</span><span class="pu">,</span>'),
    (15, '      <span class="ch">"tempo_limite"</span><span class="pu">:</span> <span class="nu">60</span><span class="pu">,</span>'),
    (16, '      <span class="ch">"somente_leitura"</span><span class="pu">:</span> <span class="kw">true</span>'),
    (17, '    <span class="pu">}</span>'),
]
margem = "".join(f'<div class="erro-n">{n}</div>' if n == 14 else f"<div>{n}</div>" for n, _ in LINHAS_JSON)
codigo = "".join(f'<div class="cod{" erro-l" if n == 14 else ""}" style="padding:0 14px">{h}</div>' for n, h in LINHAS_JSON)
MOTIVO = "JSON inválido na linha 14, coluna 7: falta uma vírgula ou um } antes de \"arquivo\""
pastilha_json = achar(r'(<span class="sk s16 tipo g2" aria-hidden="true">' + UM_SVG + r'<span class="rot">\{\}</span></span>)', json_, "Json")
pastilha_lista = achar(r'(<span class="sk s16 tipo g2" aria-hidden="true">' + UM_SVG + r'<span class="rot">\[\]</span></span>)', json_, "Json")
pastilha_txt = achar(r'(<span class="sk s16 tipo g2" aria-hidden="true">' + UM_SVG + r'<span class="rot">Aa</span></span>)', json_, "Json")
pastilha_num = achar(r'(<span class="sk s16 tipo g2" aria-hidden="true">' + UM_SVG + r'<span class="rot">12</span></span>)', json_, "Json")
seta_no = achar(r'<a class="no" href="#" style="padding-left:10px">(' + UM_SVG + ')', json_, "Json")
arvore_velha = (
    '<div class="velha">'
    f'<a class="no" href="#" style="padding-left:10px">{seta_no}{pastilha_json}<span>raiz</span><span class="q">5 chaves</span></a>'
    f'<a class="no" href="#" style="padding-left:40px">{pastilha_num}<span>versao</span></a>'
    f'<a class="no" href="#" style="padding-left:40px">{pastilha_txt}<span>tema</span></a>'
    f'<a class="no" href="#" style="padding-left:24px">{seta_no}{pastilha_lista}<span>conexoes</span><span class="q">2 itens</span></a>'
    f'<a class="no" href="#" style="padding-left:38px">{seta_no}{pastilha_json}<span>1</span><span class="q">4 chaves</span></a>'
    "</div>")
faixa_json = (
    f'<div class="faixa"><span class="migalha"><span class="sk s16 arq l4" aria-hidden="true">{doc_json}<span class="rot">JSON</span></span>'
    f'<span style="margin-left:3px">config.json</span>{seta_migalha}conexoes{seta_migalha}1{seta_migalha}<strong style="color:var(--t)">arquivo</strong></span>'
    '<span style="flex:1"></span>'
    f'<span class="estado-v" tabindex="0" role="img" aria-label="{MOTIVO}"><span class="sk s16" aria-hidden="true">{lado(erro_svg, 16)}</span>'
    f'<span class="dica-v fixa" role="tooltip">{MOTIVO}</span></span>'
    '<button class="btn sutil pq" disabled="" title="Corrija o erro para formatar">Formatar</button></div>')
n1 = estado(
    "JSON inválido",
    "O estado do documento vira o ícone de erro, com o motivo na dica e no rótulo de acessibilidade; o clique leva ao erro. "
    "A linha do erro ganha fundo e sublinhado ondulado em --perigo, e o número da margem também. Formatar fica desligado. "
    "A estrutura mostra a última versão válida, em --t3. Salvar continua permitido, e o aviso diz que salvou com erro.",
    '<div style="display:grid;grid-template-columns:minmax(0,1fr) 272px;gap:8px">'
    '<div class="doc" style="min-width:0">' + faixa_json
    + '<div style="display:flex;background:var(--c);padding-bottom:10px">'
    f'<div class="cod margem" aria-hidden="true" style="width:46px;flex:none;text-align:right;padding:10px 12px 0 0;color:var(--t3);border-right:1px solid var(--b);background:var(--p)">{margem}</div>'
    f'<div style="flex:1;min-width:0;padding:10px 0 0;overflow:hidden">{codigo}</div></div>'
    '<div class="rodape"><span>24 linhas</span><span style="color:var(--t3)">· Espaços: 2</span><span style="flex:1"></span>'
    '<span style="color:var(--t3)">Linha 14, coluna 7</span></div></div>'
    '<aside class="vidro" aria-label="Propriedades" style="border-radius:10px;overflow:hidden">'
    '<div class="cab"><span class="tit">Propriedades</span></div>'
    '<div class="sec">Estrutura do documento</div>'
    '<div class="tit" style="padding:0 14px 6px;color:var(--t3)">Última versão válida</div>'
    + arvore_velha + '</aside></div>'
    f'<div role="status" class="flut toast"><span class="sk s16" aria-hidden="true">{lado(atencao, 16)}</span>'
    'config.json salvo com o erro da linha 14.</div>',
    largo=True)

# --- N2: fechar a aba alterada -------------------------------------------------------------------------------------
abas = achar(r'(<div role="tablist" aria-label="Abas abertas"[^>]*>\n.*?\n</div>)\n<div class="doc"', csv, "Csv")
aba_csv = achar(r'(<div class="aba on" role="presentation"><a href="#" role="tab" aria-selected="true">.*?</a><span class="mod" title="Alterado, não salvo"></span></div>)', abas, "Csv")
aba_csv_foco = (aba_csv.replace('<div class="aba on" role="presentation">', '<div class="aba on foco" role="presentation">', 1)
                .replace('<span class="mod" title="Alterado, não salvo"></span></div>',
                         f'<span class="mod" title="Alterado, não salvo"></span><button class="x" aria-label="Fechar vendas_2025.csv" '
                         f'title="Fechar (Ctrl+W)">{fechar_x}</button></div>'))
abas_n2 = abas.replace(aba_csv, aba_csv_foco)
abas_n2 = re.sub(r'<button class="ib pq" aria-label="Nova consulta".*?</button>\n', "", abas_n2, flags=re.S)
dialogo = ('<section class="dlg flut" role="alertdialog" aria-label="Salvar as alterações" style="width:440px;align-self:center">'
           f'<header><span class="sk s20 arq" aria-hidden="true">{lado(doc_csv, 20)}<span class="rot" style="font-size:6.8px;letter-spacing:0">CSV</span></span>'
           '<h2 style="margin-left:10px">Salvar as alterações em vendas_2025.csv?</h2></header>'
           '<div style="padding:16px 20px 18px;color:var(--t2)">Há 1 alteração não salva. Se não salvar, ela se perde.</div>'
           '<footer><button class="btn sutil perigo">Não salvar</button><span style="flex:1"></span>'
           '<button class="btn sutil" title="Esc">Cancelar</button><button class="btn pri">Salvar</button></footer></section>')
n2 = estado(
    "Fechar a aba alterada",
    "O ponto de \"alterado\" vira o × no hover e no foco. Fechar a aba, abrir outra pasta ou fechar o app pergunta antes; "
    "Esc é Cancelar. A janela de confirmação do kit ganha uma segunda ação opcional (Não salvar).",
    '<div style="position:relative;height:36px">' + abas_n2.replace('style="height:35px;', 'style="height:35px;position:absolute;left:0;right:0;top:0;', 1) + '</div>'
    + dialogo,
    estilo_palco="min-height:300px")

# --- N4: coluna nova no CSV ----------------------------------------------------------------------------------------
nova_col_dica = (f'<span class="com-dica"><button class="ib on" aria-label="Nova coluna" title="Nova coluna depois de quantidade">{nova_coluna}</button>'
                 '<span class="dica-fixa esq" role="tooltip">Nova coluna depois de quantidade</span></span>')
th_num = '<th class="num" style="color:var(--t3);width:48px;padding:0 8px">#</th>'
mais_cabecalho = achar(r'<th class="nova-col"><button class="ib pq" aria-label="Nova coluna" title="Nova coluna no fim">(' + UM_SVG + ')</button>', csv, "Csv")
ok_toast = achar(r'<div role="status" class="flut"[^>]*>\s*<span class="sk s16" aria-hidden="true">(' + UM_SVG + ')</span>', json_, "Json")
LINHAS_CSV = [(5, "Estojo duplo", "6", "301,80"), (6, "Caneta gel", "13", "775,45"), (7, "Caderno universitário", "20", "258,00"),
              (8, "Marca-texto", "4", "89,00"), (9, "Estojo duplo", "11", "347,60")]
cab_n4 = ('<thead><tr>' + th_num
          + f'<th title="texto">{pastilha_csv("produto")}produto</th>'
          + f'<th class="num" title="número inteiro (detectado)" style="width:120px">{pastilha_csv("quantidade")}quantidade</th>'
          + '<th class="nova" style="width:130px"><span class="th-edit" role="textbox" aria-label="Nome da coluna nova" tabindex="0">'
            '<span class="sel-txt">coluna_7</span></span></th>'
          + f'<th class="num" title="número decimal (detectado)" style="width:100px">{pastilha_csv("valor")}valor</th>'
          + f'<th class="nova-col" style="width:34px"><button class="ib pq" aria-label="Nova coluna" title="Nova coluna no fim">'
            f'{mais_cabecalho}</button></th></tr></thead>')
corpo_n4 = "".join(
    f'<tr><td class="lin">{n}</td><td>{p}</td><td class="num{" ativa" if n == 7 else ""}">{q}</td><td class="nova"></td><td class="num">{v}</td><td></td></tr>'
    for n, p, q, v in LINHAS_CSV)
n4 = estado(
    "Nova coluna no CSV",
    "O botão da barra insere depois da coluna da célula ativa (a dica diz onde); o + do cabeçalho insere no fim. A coluna "
    "nasce como coluna_7, com o cabeçalho já em edição no lugar (raio 4, texto selecionado): Enter confirma e Esc desfaz. "
    "A coluna nova conta como alteração não salva.",
    barra(botao(csv, "Nova linha", "Csv"), nova_col_dica, botao(csv, "Duplicar", "Csv"), botao(csv, "Excluir", "Csv"))
    + '<div class="doc" style="margin-top:26px"><table class="grade">' + cab_n4 + "<tbody>" + corpo_n4 + "</tbody></table>"
    + '<div class="rodape"><span>1.204 linhas · 7 colunas</span><span style="color:var(--acct)">· 1 alteração não salva</span></div></div>')

# --- D9: várias linhas marcadas no CSV -----------------------------------------------------------------------------
excluir_dica = (f'<span class="com-dica"><button class="ib" aria-label="Excluir" title="Excluir as 2 linhas marcadas">{lixeira}</button>'
                '<span class="dica-fixa esq" role="tooltip">Excluir as 2 linhas marcadas</span></span>')
duplicar_marcadas = botao(csv, "Duplicar", "Csv").replace('title="Duplicar a linha 7"', 'title="Duplicar as 2 linhas marcadas"')
LINHAS_MARC = [(3, "2025-01-04", "Nordeste", "15", False), (4, "2025-01-05", "Norte", "22", True), (5, "2025-01-06", "Centro-Oeste", "6", True),
               (6, "2025-01-07", "Sul", "13", False), (7, "2025-01-08", "Sudeste", "20", False)]
cab_marc = ('<thead><tr><th class="selc"><input type="checkbox" aria-label="Marcar a página"></th>' + th_num
            + f'<th title="data (detectado)">{pastilha_csv("data")}data</th><th title="texto">{pastilha_csv("regiao")}regiao</th>'
            + f'<th class="num" title="número inteiro (detectado)">{pastilha_csv("quantidade")}quantidade</th></tr></thead>')
corpo_marc = "".join(
    f'<tr class="{"mar" if m else ""}"><td class="selc"><input type="checkbox"{" checked=\"\"" if m else ""} aria-label="Marcar a linha {n}"></td>'
    f'<td class="lin">{n}</td><td>{d}</td><td>{r}</td><td class="num">{q}</td></tr>' for n, d, r, q, m in LINHAS_MARC)
d9_marcar = estado(
    "Várias linhas marcadas no CSV",
    "A coluna de marcar (32 px) igual à dos Dados. Com linhas marcadas, Duplicar e Excluir agem nelas e a dica diz o alvo; "
    "sem marcas, agem na linha da célula ativa.",
    barra(botao(csv, "Nova linha", "Csv"), botao(csv, "Nova coluna", "Csv"), duplicar_marcadas, excluir_dica)
    + '<div class="doc" style="margin-top:26px"><table class="grade">' + cab_marc + "<tbody>" + corpo_marc + "</tbody></table>"
    + '<div class="rodape"><span>1.204 linhas · 6 colunas</span><span style="color:var(--t3)">· 2 marcadas</span></div></div>')

# --- D9: menu da coluna no CSV -------------------------------------------------------------------------------------
cab_menu = ('<thead><tr>' + th_num + f'<th title="texto">{pastilha_csv("produto")}produto</th>'
            + f'<th class="num" title="número inteiro (detectado)" style="width:130px;background:var(--tint)">{pastilha_csv("quantidade")}quantidade</th>'
            + f'<th class="num" title="número decimal (detectado)" style="width:110px">{pastilha_csv("valor")}valor</th></tr></thead>')
corpo_menu = "".join(f'<tr><td class="lin">{n}</td><td>{p}</td><td class="num">{q}</td><td class="num">{v}</td></tr>' for n, p, q, v in LINHAS_CSV[:4])
menu_coluna = ('<div role="menu" aria-label="Coluna quantidade" class="flut menu-ctx" style="left:300px;top:52px">'
               f'<a class="mi" role="menuitem" href="#">{renomear}Renomear a coluna<span class="at">F2</span></a>'
               f'<a class="mi" role="menuitem" href="#">{nova_coluna}Nova coluna depois desta</a>'
               '<div class="msep"></div>'
               f'<a class="mi perigo" role="menuitem" href="#">{lixeira}Excluir a coluna</a></div>')
d9_coluna = estado(
    "Renomear e excluir coluna no CSV",
    "Pelo menu do cabeçalho (clique direito ou Shift+F10) e pela seção da coluna nas Propriedades. Renomear abre o "
    "cabeçalho em edição, como na coluna nova; excluir entra no desfazer e conta como alteração não salva.",
    '<div class="doc"><table class="grade">' + cab_menu + "<tbody>" + corpo_menu + "</tbody></table></div>" + menu_coluna,
    estilo_palco="min-height:250px")

# --- N5: Histórico vazio -------------------------------------------------------------------------------------------
# o Histórico da biblioteca em 48 px (o desenho de 48, sem o contorno firme dos tamanhos pequenos)
historico_icone = achar(r'<sc-if value="\{\{z\.a_historico\}\}"[^>]*>(' + UM_SVG + ')</sc-if>', icones, "Icones")
historico_icone = re.sub(r' class="cont(?:-p)?"', "", historico_icone.replace('width="{{z.s}}" height="{{z.s}}"', 'width="48" height="48"'))
n5 = estado(
    "Histórico vazio",
    "Como o Explorador do Início. O clique num item seleciona; o duplo clique ou Enter abre o SQL numa Consulta nova.",
    '<aside class="vidro" aria-label="Histórico de consultas" style="width:258px;height:300px;border-radius:10px;display:flex;flex-direction:column;overflow:hidden;align-self:center">'
    '<div class="cab"><span class="tit">Histórico</span></div>'
    f'<div class="vazio"><span class="sk s48" aria-hidden="true">{historico_icone}</span>'
    '<div style="color:var(--t2)">Nenhuma consulta ainda.<br><span style="color:var(--t3)">As consultas executadas aparecem aqui.</span></div></div></aside>',
    estilo_palco="min-height:330px")

# --- D9: Copiar caminho no Explorador ------------------------------------------------------------------------------
cab_explorador = achar(r'(<div class="cab"><span class="tit">Explorador</span>.*?</div>)', csv, "Csv")
nos = achar(r'(<div class="no" style="padding-left:8px" title="C:\\dados">.*?)</div>\n</aside>', csv, "Csv")
menu_arquivo = ('<div role="menu" aria-label="vendas_2025.csv" class="flut menu-ctx" style="left:200px;top:120px">'
                '<a class="mi" role="menuitem" href="#"><span style="width:14px"></span>Abrir</a>'
                f'<a class="mi" role="menuitem" href="#">{mostrar_na_pasta}Mostrar na pasta</a>'
                f'<a class="mi" role="menuitem" href="#">{copiar_caminho}Copiar caminho<span class="at">Ctrl+Shift+C</span></a></div>')
d9_caminho = estado(
    "Copiar caminho",
    "No menu do item no Explorador e na seção Arquivo das Propriedades. Copia o caminho completo; o aviso flutuante confirma.",
    '<aside class="vidro" aria-label="Explorador" style="width:258px;border-radius:10px;overflow:hidden;padding-bottom:8px">'
    + cab_explorador + '<div style="padding:6px 0 0">' + nos + "</div></aside>" + menu_arquivo
    + f'<div role="status" class="flut toast" style="position:absolute;right:20px;bottom:18px"><span class="sk s16" aria-hidden="true">'
      f'{ok_toast}</span>Caminho copiado.</div>',
    estilo_palco="min-height:330px")

# --- D9: ficha da linha só leitura ---------------------------------------------------------------------------------
ficha = achar(r'(<section class="dlg flut larga" role="dialog" aria-label="Linha P003">.*?</section>)', janelas, "Janelas")
so = ficha.replace('aria-label="Linha P003"', 'aria-label="Linha P003, só leitura"', 1)
so = so.replace('<span style="color:var(--t3);margin-left:8px">produtos</span>',
                '<span style="color:var(--t3);margin-left:8px">produtos · só leitura</span>', 1)
so, n = re.subn(r'<input class="campo( nulo)?"', lambda m: f'<input class="campo so-leitura{m.group(1) or ""}" readonly=""', so)
assert n >= 5, ("campos da ficha", n)
so = re.sub(r'<input type="checkbox"( checked="")? aria-label', r'<input type="checkbox"\1 disabled="" aria-label', so)
so = so.replace('<button class="btn pq">Substituir por arquivo</button>', "")
so = re.sub(r'<footer>.*?</footer>', '<footer><span style="flex:1"></span><button class="btn">Copiar a linha</button>'
            '<button class="btn pri">Fechar</button></footer>', so, flags=re.S)
so = so.replace('<div class="ficha">', f'<div class="nota-j"><span class="sk s24" aria-hidden="true">{cadeado(24)}</span>'
                '<span>exemplo.db está aberto só para leitura. Nada é gravado no arquivo.</span></div><div class="ficha">', 1)
d9_ficha = estado(
    "Ficha da linha só leitura",
    "Para planilhas do Excel e bancos abertos só para leitura: os campos ficam só leitura (opacos, sem baixo-relevo), o NULL "
    "não se marca, o BLOB só se salva, e o rodapé fica com Copiar a linha e Fechar.",
    so, largo=True, estilo_palco="align-items:center;padding:24px")

quadro = '<div class="quadro">' + n3 + n1 + n2 + n4 + d9_marcar + d9_coluna + n5 + d9_caminho + d9_ficha + "</div>"
corpo = ('<div class="est {{classes}}" style="width:100%;min-height:2960px;position:relative;background:var(--ambiente)">\n'
         + defs + "\n" + quadro + "\n</div>")
# o script do Exportar só monta as classes do tema e do destaque
script = exportar[exportar.index('<script type="text/x-dc"'):]
script = re.sub(r'"\$preview":\{"width":\d+,"height":\d+\}', '"$preview":{"width":1440,"height":2960}', script)
pagina = cabeca + "</helmet>\n" + corpo + "\n</x-dc>\n" + script
(PASTA / "Estados.dc.html").write_text(pagina, encoding="utf-8", newline="\n")
print("Estados.dc.html", len(pagina), "bytes")

# o lugar da prancheta no canvas: abaixo da 14, na mesma coluna
lay = PASTA / "canvas.json"
j = json.loads(lay.read_text(encoding="utf-8"))
entrada = {"expand": "fill", "h": 2960, "title": "15 · Estados a implementar (escuro)", "w": 1440, "x": 4600, "y": 6620}
mudou = j["boards"].get("Estados.dc.html") != entrada or "Estados.dc.html" not in j["order"]
j["boards"]["Estados.dc.html"] = entrada
if "Estados.dc.html" not in j["order"]:
    j["order"].append("Estados.dc.html")
if mudou:
    lay.write_text(json.dumps(j, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print("canvas.json: prancheta 15 em x 4600, y 6620")
