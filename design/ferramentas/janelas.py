"""Prancheta 12: as três janelas pesadas — a janela da linha (com BLOB e NULL), a de coluna (Alterar a coluna) e a
Nova tabela — no mesmo sistema das outras pranchetas (folha de estilo, ícones e fundo do Exportar.dc.html; o
triângulo de atenção do DadosClaro.dc.html). Gera ../canvas/Janelas.dc.html e refaz a prancheta inteira.
Os ajustes de 15/10/2026 (raio único 10, campos retangulares, seletor igual, caixas quadradas) e os da revisão de
15/10/2026 que valem para todas as telas chegam pela folha do Exportar: rode antes os geradores revisao_*.py.

Revisão de 15/10/2026, aprovada em 16/10/2026, aplicada aqui:
- 9: a janela da linha mostra produtos P003 (a do exemplo.db com BLOB), pastilha pelo tipo declarado, valores crus
  como na grade, a coluna gerada como campo só leitura (opaco, sem baixo-relevo) e o NULL em itálico num campo normal;
- 10: controles de verdade (input, button com aria-haspopup, label for) e um nome em cada caixa de marcar;
- 11: as cores do SQL e do aviso nesta prancheta;
- 12: a prévia mostra o plano que o app gera, um comando por linha, com altura máxima e rolagem; o aviso é o texto
  que o app produz; o campo Referência sem o invólucro;
- 21: "Chave primária" e "Não aceita nulo" em duas linhas na Nova tabela (coluna de 80) e "Ordenação (COLLATE)";
- pedido de 16/10/2026: as pastilhas de tipo da ficha da linha com 24 px (os campos de tipo continuam em 16).
Uso: python janelas.py [pasta das pranchetas]"""
import re
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
PASTA = Path(sys.argv[1]) if len(sys.argv) > 1 else AQUI.parent / "canvas"
sys.stdout.reconfigure(encoding="utf-8")

base = (PASTA / "Exportar.dc.html").read_text(encoding="utf-8")
cabeca = base[: base.index("</helmet>")]
estilo_extra = """.est{--kw:#8ab4f8;--nu:#f2b65e;--st:#7fd1a8;--aviso:#f2b65e}.est.claro{--kw:#1f5fbf;--nu:#a35200;--st:#0f7a4c;--aviso:#8f5400}
.cpo{display:flex;flex-direction:column;gap:6px;min-width:0}
.cpo.largo{grid-column:span 2}
.rc{display:flex;align-items:center;gap:7px;color:var(--t);font-weight:600;white-space:nowrap}
.rc .rg{font-weight:400;color:var(--t3);font-size:11px;overflow:hidden;text-overflow:ellipsis}
.nulo-c{display:flex;align-items:center;gap:6px;color:var(--t3);font-size:11px}
.nulo-c input{width:14px;height:14px}
.grade2{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);gap:14px 16px}
.cod{font-family:Consolas,"Cascadia Mono",monospace;font-size:12.5px;line-height:20px;white-space:pre-wrap}
.prev{padding:10px 12px;border-radius:10px;background:var(--f);border:1px solid var(--fb);box-shadow:inset 0 1px 2px rgba(0,0,0,.3)}
.prev.plano{max-height:244px;overflow-y:auto}
.prev .cmd{display:block;padding-left:2ch;text-indent:-2ch}
.prev .cmd.dentro{padding-left:4ch}
.kw{color:var(--kw)}.nu{color:var(--nu)}.st{color:var(--st)}.pu{color:var(--t3)}
.aviso-j{display:flex;align-items:center;gap:12px;padding:9px 12px;border-radius:10px;width:fit-content;max-width:min(100%,560px);margin-inline:auto;text-wrap:pretty;background:color-mix(in oklab,var(--aviso) 12%,transparent);border:1px solid color-mix(in oklab,var(--aviso) 35%,transparent)}
.lf{display:grid;grid-template-columns:minmax(0,.95fr) minmax(0,1.15fr) minmax(0,1.4fr) 84px 80px 52px 28px;gap:8px;align-items:center}
.lf .tx{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.lf .h{font-size:11px;font-weight:600;color:var(--t3);letter-spacing:.06em;text-transform:uppercase;white-space:nowrap;line-height:1.3}
.lf .ck{display:flex;justify-content:center}
.campo.pq{height:26px}
.dlg.larga{width:760px}
.pill{display:inline-block;font-size:10px;font-weight:700;letter-spacing:.05em;text-transform:uppercase;padding:1px 7px;border-radius:999px;background:var(--h);color:var(--t2);white-space:nowrap}
.btn.pq{height:26px;padding:0 12px}
.btn.sutil{background:transparent;border-color:transparent;box-shadow:none;color:var(--t2)}
.btn.sutil.perigo{color:var(--perigo)}
.ficha{display:flex;flex-direction:column;margin:-6px 0}
.fl{display:grid;grid-template-columns:250px minmax(0,1fr) 56px;gap:0 12px;align-items:center;padding:9px 2px;border-bottom:1px solid var(--b)}
.fl:last-child{border-bottom:0}
.fl-cab{padding:0 2px 6px;font-size:11px;font-weight:600;color:var(--t3);letter-spacing:.06em;text-transform:uppercase;border-bottom-color:var(--vidrob)}
.fl-nome{display:flex;flex-direction:column;gap:2px;min-width:0}
.fl-t{display:flex;align-items:center;gap:8px;color:var(--t);font-weight:600;white-space:nowrap;cursor:default}
.fl-r{padding-left:32px;color:var(--t3);font-size:11px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.fl-v{min-width:0}
.fl-v>.campo,.cpo>.campo{display:flex;width:100%}
.nulo-f{display:flex;align-items:center;justify-content:center;gap:6px;color:var(--t3);font-size:11px}
.dlg.larga .ficha+*{margin-top:0}
.sk.s24.tipo .rot{left:0;top:0;width:100%;height:100%}
.sk.s24.tipo.g1 .rot{font-size:15px}.sk.s24.tipo.g2 .rot{font-size:11px}.sk.s24.tipo.g3 .rot{font-size:9.5px}
input.campo,textarea.campo{width:100%;cursor:text;outline:none;font:inherit}
.campo::placeholder{color:var(--t3);opacity:1}
.campo.nulo::placeholder{font-style:italic}
.campo.so-leitura{background:var(--p);box-shadow:none;color:var(--t2);border-color:var(--fb);cursor:default}
.est.claro .campo.so-leitura{box-shadow:none}
"""
cabeca = cabeca.replace("</style>", estilo_extra + "</style>", 1)


def icone_da_lista(tabela, lado):
    """O ícone de uma tabela da lista "Tabelas" do Exportar o banco (em 16 px), levado a outro tamanho. Vem da
    lista, e não do cabeçalho do Exportar: se o cabeçalho ganhar outro ícone, esta prancheta não muda junto
    (achado no teste da skill icones-databox, 15/10/2026)."""
    m = re.search(r'<label class="op"><input type="checkbox" checked=""><span class="sk s16" aria-hidden="true">'
                  r'(<svg viewBox="0 0 48 48" width="16" height="16">(?:(?!</svg>).)*</svg>)</span>' + re.escape(tabela) + '</label>',
                  base, re.S)
    if not m:
        raise SystemExit(f"Exportar: não achei a tabela {tabela} na lista do Exportar o banco")
    return m.group(1).replace('width="16" height="16"', f'width="{lado}" height="{lado}"', 1)


icone_clientes = icone_da_lista("clientes", 20)
icone_produtos = icone_da_lista("produtos", 20)
fechar = re.search(r'<button class="ib" aria-label="Fechar">(<svg.*?</svg>)</button>', base, re.S).group(1)
seta = re.search(r'<button class="campo" aria-label="Separador"[^>]*>[^<]*<span style="flex:1"></span>(<svg.*?</svg>)</button>', base, re.S).group(1)
dados_claro = (PASTA / "DadosClaro.dc.html").read_text(encoding="utf-8")
atencao = re.search(r'<div class="aviso" role="alert">\s*<span class="sk s\d+" aria-hidden="true">(<svg.*?</svg>)</span>', dados_claro, re.S).group(1)
# o triângulo dos avisos em 24 px, centrado na altura do texto (pedido de 15/10/2026)
atencao = re.sub(r'width="\d+" height="\d+"', 'width="24" height="24"', atencao, count=1)
defs = re.search(r'<svg width="0" height="0"[^>]*><defs>.*?</defs></svg>', base, re.S).group(0)

# cores escurecidas na revisão de 15/10/2026 (item 3b); NUMERIC usa a pastilha do REAL
TIPOS = {"INTEGER": ("#2c6fdc", "12", 2), "REAL": ("#1d8181", ",5", 2), "TEXT": ("#5f6b7a", "Aa", 2),
         "BLOB": ("#7a52c7", "01", 2), "DATA": ("#c8443b", "31", 2), "BOOL": ("#27854c", "✓", 1),
         "PK": ("#927010", "PK", 2), "FK": ("#4a6fd1", "FK", 2)}


def pastilha(tipo, lado=16):
    """Pastilha de tipo. Até 24 px, contorno do corpo com 2,4 e .35 (revisão, item 13) e o realce de cima que
    termina antes do glifo (item 3a)."""
    cor, glifo, g = TIPOS[tipo]
    return (f'<span class="sk s{lado} tipo g{g}" aria-hidden="true"><svg viewBox="0 0 48 48" width="{lado}" height="{lado}">'
            f'<rect filter="url(#icSombra)" x="4" y="4" width="40" height="40" rx="11" fill="{cor}" stroke="#000000" '
            f'style="stroke-opacity:.35;stroke-width:2.4"></rect><rect x="10" y="5.2" width="28" height="6.8" rx="3.4" fill="url(#icRealce)">'
            f'</rect><rect x="4" y="26" width="40" height="18" rx="11" fill="url(#icFundo)"></rect></svg>'
            f'<span class="rot">{glifo}</span></span>')


def entrada(ident, valor="", extra="", classe="campo", rotulo=None, dica=None):
    aria = f' aria-label="{rotulo}"' if rotulo else ""
    titulo = f' title="{dica}"' if dica else ""
    v = f' value="{valor}"' if valor != "" else ""
    i = f' id="{ident}"' if ident else ""
    return f'<input class="{classe}"{i}{v}{aria}{titulo}{extra}>'


def lista(ident_rotulo, conteudo, tipo=None, vazio=False, largura=None, pq=False, rotulo=None):
    """Lista suspensa: um botão com aria-haspopup (um <select> perderia a pastilha)."""
    estilo = []
    if tipo:
        estilo.append("gap:8px")
    if vazio:
        estilo.append("color:var(--t3)")
    st = f' style="{";".join(estilo)}"' if estilo else ""
    nome = f' aria-labelledby="{ident_rotulo}"' if ident_rotulo else f' aria-label="{rotulo}"'
    icone = pastilha(tipo) if tipo else ""
    classe = "campo pq" if pq else "campo"
    texto = f'<span class="tx">{conteudo}</span>' if pq else f"<span>{conteudo}</span>"
    return f'<button class="{classe}" aria-haspopup="listbox"{nome}{st}>{icone}{texto}<span style="flex:1"></span>{seta}</button>'


def janela(titulo, sub, corpo, rodape, icone, larga=False):
    classe = "dlg flut larga" if larga else "dlg flut"
    sub_html = f'<span style="color:var(--t3);margin-left:8px">{sub}</span>' if sub else ""
    return (f'<section class="{classe}" role="dialog" aria-label="{titulo}">'
            f'<header><span class="sk s20" aria-hidden="true">{icone}</span><h2 style="margin-left:10px">{titulo}</h2>'
            f'{sub_html}<span style="flex:1"></span><button class="ib" aria-label="Fechar" title="Fechar (Esc)">{fechar}</button></header>'
            f'<div style="padding:18px 20px 20px;display:flex;flex-direction:column;gap:16px">{corpo}</div>'
            f'<footer>{rodape}</footer></section>')


def ficha(tipo, nome, regra, controle, nulo=None, dica_regra=None):
    """Uma linha da ficha da janela da linha: pastilha de 24 px, nome (o rótulo do campo) e regras à esquerda; o
    valor no meio; o NULL numa coluna própria à direita (vazia quando a coluna não aceita nulo); fio embaixo."""
    if nulo is None:
        n = '<span></span>'
    else:
        marc = ' checked=""' if nulo else ""
        n = f'<label class="nulo-f"><input type="checkbox"{marc} aria-label="{nome} é NULL">NULL</label>'
    titulo = f' title="{dica_regra}"' if dica_regra else ""
    return (f'<div class="fl"><div class="fl-nome"><label class="fl-t" for="f-{nome}">{pastilha(tipo, 24)}{nome}</label>'
            f'<span class="fl-r"{titulo}>{regra}</span></div><div class="fl-v">{controle}</div>{n}</div>')


# 9a: produtos P003 do exemplo.db (codigo TEXT PRIMARY KEY, WITHOUT ROWID; foto BLOB de 112 bytes)
linha = janela(
    "Linha P003", "produtos",
    '<div class="ficha">'
    '<div class="fl fl-cab"><span>Coluna</span><span>Valor</span><span style="text-align:center">NULL</span></div>'
    + ficha("PK", "codigo", "TEXT · chave primária", entrada("f-codigo", "P003"))
    + ficha("TEXT", "descricao", "TEXT · não aceita nulo · ordenação NOCASE", entrada("f-descricao", "Grampeador"))
    + ficha("REAL", "preco", "NUMERIC · não aceita nulo · padrão 0", entrada("f-preco", "22.39"))
    + ficha("INTEGER", "estoque", "INTEGER · padrão 0", entrada("f-estoque", "493"), nulo=False)
    + ficha("BLOB", "foto", "BLOB",
            '<div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap" id="f-foto" role="group" aria-label="foto">'
            '<span class="pill" title="BLOB de 112 bytes">112 bytes</span>'
            '<button class="btn pq">Salvar em arquivo</button><button class="btn pq">Substituir por arquivo</button></div>',
            nulo=False)
    + ficha("INTEGER", "serie", "INTEGER", entrada("f-serie", classe="campo nulo", extra=' placeholder="NULL"'), nulo=True)
    + ficha("INTEGER", "preco_centavos", "INTEGER · gerada (VIRTUAL)",
            entrada("f-preco_centavos", "2239", classe="campo so-leitura", extra=" readonly",
                    dica="Coluna gerada: o valor sai de CAST(preco * 100 AS INTEGER) e não se edita"),
            dica_regra="GENERATED ALWAYS AS (CAST(preco * 100 AS INTEGER)) VIRTUAL")
    + "</div>",
    '<button class="btn sutil perigo">Excluir linha</button><button class="btn">Duplicar</button>'
    '<span style="flex:1"></span><button class="btn sutil">Cancelar</button><button class="btn pri">Salvar</button>',
    icone_produtos, larga=True)


def cmd(html, dentro=False):
    return f'<span class="cmd{" dentro" if dentro else ""}">{html}</span>'


def ident(nome):
    return f'<span class="st">"{nome}"</span>'


COLUNAS = ["id", "nome", "email", "municipio", "limite", "ativo", "criado_em"]
lista_colunas = '<span class="pu">, </span>'.join(ident(c) for c in COLUNAS)
# 12: o plano que o app gera (esquema::colunas::plano_alterar_coluna → reconstrução de 12 passos), um comando por linha
sql_coluna = "".join([
    cmd('<span class="kw">BEGIN</span><span class="pu">;</span>'),
    cmd(f'<span class="kw">ALTER TABLE</span> {ident("clientes")} <span class="kw">RENAME COLUMN</span> {ident("cidade")} '
        f'<span class="kw">TO</span> {ident("municipio")}<span class="pu">;</span>'),
    cmd(f'<span class="kw">CREATE TABLE</span> {ident("_novo_clientes")} <span class="pu">(</span>'),
    cmd(f'{ident("id")} <span class="kw">INTEGER PRIMARY KEY AUTOINCREMENT</span><span class="pu">,</span>', True),
    cmd(f'{ident("nome")} <span class="kw">TEXT NOT NULL</span><span class="pu">,</span>', True),
    cmd(f'{ident("email")} <span class="kw">TEXT UNIQUE</span><span class="pu">,</span>', True),
    cmd(f'{ident("municipio")} <span class="kw">TEXT NOT NULL CHECK</span> <span class="pu">(</span>length<span class="pu">(</span>municipio'
        f'<span class="pu">)</span> <span class="pu">&gt;</span> <span class="nu">1</span><span class="pu">)</span> '
        f'<span class="kw">DEFAULT</span> <span class="st">\'São Paulo\'</span> <span class="kw">COLLATE NOCASE</span><span class="pu">,</span>', True),
    cmd(f'{ident("limite")} <span class="kw">REAL CHECK</span> <span class="pu">(</span>limite <span class="pu">&gt;=</span> '
        f'<span class="nu">0</span><span class="pu">),</span>', True),
    cmd(f'{ident("ativo")} <span class="kw">INTEGER NOT NULL DEFAULT</span> <span class="nu">1</span><span class="pu">,</span>', True),
    cmd(f'{ident("criado_em")} <span class="kw">TEXT DEFAULT</span> CURRENT_TIMESTAMP', True),
    cmd('<span class="pu">);</span>'),
    cmd(f'<span class="kw">INSERT INTO</span> {ident("_novo_clientes")} <span class="pu">(</span>{lista_colunas}<span class="pu">)</span>'),
    cmd(f'<span class="kw">SELECT</span> {lista_colunas} <span class="kw">FROM</span> {ident("clientes")}<span class="pu">;</span>', True),
    cmd('<span class="pu">-- copia o contador do AUTOINCREMENT (sqlite_sequence)</span>'),
    cmd(f'<span class="kw">DROP VIEW</span> {ident("v_pedidos_por_cliente")}<span class="pu">;</span>'),
    cmd(f'<span class="kw">DROP TABLE</span> {ident("clientes")}<span class="pu">;</span>'),
    cmd(f'<span class="kw">ALTER TABLE</span> {ident("_novo_clientes")} <span class="kw">RENAME TO</span> {ident("clientes")}<span class="pu">;</span>'),
    cmd('<span class="pu">-- recria o índice idx_clientes_nome_cidade e a visão v_pedidos_por_cliente</span>'),
    cmd(f'<span class="kw">PRAGMA</span> foreign_key_check<span class="pu">(</span>{ident("clientes")}<span class="pu">);</span>'),
    cmd(f'<span class="kw">PRAGMA</span> foreign_key_check<span class="pu">(</span>{ident("pedidos")}<span class="pu">);</span>'),
    cmd('<span class="kw">COMMIT</span><span class="pu">;</span>'),
])
# o primeiro aviso que o app junta a toda reconstrução (esquema::plano::AVISO_RECONSTRUCAO); a cidade não tem
# nulos no exemplo.db, então o aviso "Há N linhas com cidade nulo…" não aparece nesta mudança
AVISO_RECONSTRUCAO = ("A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os "
                      "índices, gatilhos e visões ligados a ela são recriados.")
coluna = janela(
    "Alterar a coluna cidade", "clientes",
    '<div class="grade2">'
    + '<label class="cpo"><span class="tit">Nome</span>' + entrada("col-nome", "municipio") + "</label>"
    + '<div class="cpo"><span class="tit" id="col-tipo">Tipo</span>' + lista("col-tipo", "TEXT", tipo="TEXT") + "</div>"
    + '<label class="cpo"><span class="tit">Valor padrão</span>' + entrada("col-padrao", "'São Paulo'") + "</label>"
    + '<label class="cpo"><span class="tit">Verificação (CHECK)</span>' + entrada("col-check", "length(municipio) &gt; 1") + "</label>"
    + '<div class="cpo"><span class="tit" id="col-ordem">Ordenação (COLLATE)</span>' + lista("col-ordem", "NOCASE") + "</div>"
    + '<div class="cpo"><span class="tit" id="col-ref">Referência</span>' + lista("col-ref", "nenhuma", vazio=True) + "</div>"
    + "</div>"
    + '<div style="display:flex;flex-wrap:wrap;gap:10px 18px">'
    '<label class="op"><input type="checkbox" checked="">Não aceita nulo</label>'
    '<label class="op"><input type="checkbox">Único</label>'
    '<label class="op"><input type="checkbox">Chave primária</label>'
    '<label class="op" style="opacity:.5"><input type="checkbox" disabled="">Autoincremento</label></div>'
    + f'<div class="blk"><span class="tit">SQL que será executado</span><div class="prev cod plano" tabindex="0" '
      f'aria-label="SQL que será executado">{sql_coluna}</div></div>'
    + f'<div class="aviso-j"><span class="sk s24" aria-hidden="true">{atencao}</span><span>{AVISO_RECONSTRUCAO}</span></div>',
    '<button class="btn sutil perigo">Excluir coluna</button><span style="flex:1"></span>'
    '<button class="btn sutil">Cancelar</button><button class="btn pri">Aplicar</button>',
    icone_clientes)


TIPO_DE = {"INTEGER": "INTEGER", "REAL": "REAL", "TEXT": "TEXT", "DATETIME": "DATA", "BLOB": "BLOB"}


def linha_nova(nome, tipo, padrao, pk, nn, un, vazio_padrao=False):
    def ck(m, regra):
        return f'<span class="ck"><input type="checkbox"{" checked=\"\"" if m else ""} aria-label="{nome}: {regra}"></span>'
    if vazio_padrao:
        padrao_html = entrada(None, classe="campo pq", rotulo=f"Padrão de {nome}", extra=' placeholder="padrão"')
    else:
        padrao_html = entrada(None, padrao, classe="campo pq", rotulo=f"Padrão de {nome}")
    tipo_html = (f'<button class="campo pq" aria-haspopup="listbox" aria-label="Tipo de {nome}" style="gap:8px">'
                 f'{pastilha(TIPO_DE[tipo])}<span class="tx">{tipo}</span><span style="flex:1"></span>{seta}</button>')
    return (entrada(None, nome, classe="campo pq", rotulo="Nome da coluna") + tipo_html + padrao_html
            + ck(pk, "chave primária") + ck(nn, "não aceita nulo") + ck(un, "único")
            + f'<button class="ib pq" aria-label="Tirar a coluna {nome}">{fechar}</button>')


sql_tabela = ('<span class="kw">CREATE TABLE</span> <span class="st">"contratos"</span> <span class="pu">(</span>\n'
              '  <span class="st">"id"</span> <span class="kw">INTEGER PRIMARY KEY</span><span class="pu">,</span>\n'
              '  <span class="st">"cliente_id"</span> <span class="kw">INTEGER NOT NULL</span><span class="pu">,</span>\n'
              '  <span class="st">"valor"</span> <span class="kw">REAL DEFAULT</span> <span class="nu">0</span><span class="pu">,</span>\n'
              '  <span class="st">"assinado_em"</span> <span class="kw">DATETIME DEFAULT</span> CURRENT_TIMESTAMP\n'
              '<span class="pu">);</span>')
nova = janela(
    "Nova tabela", "",
    '<label class="cpo" style="max-width:340px"><span class="tit">Nome da tabela</span>' + entrada("t-nome", "contratos") + "</label>"
    + '<div class="blk"><span class="tit">Colunas</span><div class="lf">'
    '<span class="h">Nome</span><span class="h">Tipo</span><span class="h">Padrão</span>'
    '<span class="h" style="text-align:center">Chave<br>primária</span><span class="h" style="text-align:center">Não aceita<br>nulo</span>'
    '<span class="h" style="text-align:center">Único</span><span></span>'
    + linha_nova("id", "INTEGER", "", True, False, False, vazio_padrao=True)
    + linha_nova("cliente_id", "INTEGER", "", False, True, False, vazio_padrao=True)
    + linha_nova("valor", "REAL", "0", False, False, False)
    + linha_nova("assinado_em", "DATETIME", "CURRENT_TIMESTAMP", False, False, False)
    + '</div><div style="display:flex;align-items:center;gap:16px;margin-top:6px"><button class="btn pq">+ Coluna</button>'
    '<label class="op"><input type="checkbox">Sem rowid (WITHOUT ROWID)</label></div></div>'
    + f'<div class="blk"><span class="tit">SQL que será executado</span><div class="prev cod">{sql_tabela}</div></div>',
    '<span style="flex:1"></span><button class="btn sutil">Cancelar</button><button class="btn pri">Criar tabela</button>',
    icone_clientes, larga=True)

inicio_fundo = base.index('<div aria-hidden="true" style="flex:1;display:flex;flex-direction:column">')
fim_fundo = base.index('<div style="position:absolute;inset:0;background:var(--veu)')
fundo_html = base[inicio_fundo:fim_fundo]

corpo = f"""<div class="est {{{{classes}}}}" style="width:100%;min-height:1500px;height:1500px;display:flex;flex-direction:column;overflow:hidden;position:relative">
{defs}
{fundo_html}
<div style="position:absolute;inset:0;background:var(--veu);display:flex;flex-direction:column;align-items:center;justify-content:flex-start;gap:28px;padding:36px 24px">
<div style="display:flex;gap:28px;align-items:flex-start">
{linha}
{coluna}
</div>
{nova}
</div>
</div>"""

script = base[base.index('<script type="text/x-dc"'):]
script = script.replace('"$preview":{"width":1440,"height":900}', '"$preview":{"width":1440,"height":1500}')
pagina = (cabeca.replace("<title>Janela Exportar</title>", "<title>Janelas: linha, coluna e Nova tabela</title>")
          + "</helmet>\n" + corpo + "\n</x-dc>\n" + script)
(PASTA / "Janelas.dc.html").write_text(pagina, encoding="utf-8", newline="\n")
print("Janelas.dc.html", len(pagina), "bytes")
