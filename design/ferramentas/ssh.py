"""A prancheta do SSH na 0.3.0 (`FuturoSsh.dc.html`, 26/10/2026): o S1 de `design/planos-ssh.md`, só leitura.

Cenas, cada uma com título e legenda, como nas pranchetas 15 a 17:
- (a) Arquivo › Conectar por SSH, no menu;
- (b) a janela Conectar por SSH: o destino com os `Host` do ~/.ssh/config, usuário, porta, pasta inicial; conectando;
  a senha e a frase da chave só quando pedidas, sem "Lembrar";
- (c) a impressão digital da chave da máquina: máquina nova (SHA-256 para conferir) e chave mudada (recusada);
- (d) o Explorador com a raiz remota, a dica da máquina e a aba "saida.csv · servidor", só leitura;
- (e) trazer o arquivo: o progresso com Interromper, o aviso acima de 1 GB e o interrompido;
- (f) o banco SQLite remoto, aberto como cópia só leitura;
- (g) o menu da raiz remota e a conexão que cai.
O ícone novo é a Máquina remota (`a_maquina`, `a_maquinaG`), posto na seção Aplicativo da biblioteca (prancheta 9) se
ainda não estiver lá; o resto vem das pranchetas. Refaz a prancheta inteira; recusa gravar por cima de um
FuturoSsh.dc.html sem a marca dele e um lugar ocupado no canvas.json; recusa a biblioteca com só uma das duas formas
ou com a forma mudada à mão.
Uso: python -I -X utf8 ssh.py"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pecas as p  # noqa: E402

MARCA = "/* prancheta do SSH, S1 da 0.3.0 (ssh.py) */"
ARQUIVO = "FuturoSsh.dc.html"
ALTURA = 4656
TITULO = "SSH na 0.3.0: conectar, a chave da máquina e o Explorador remoto (só leitura)"
LUGAR = {"x": 4560, "y": 0}

# --- o ícone novo: a máquina remota -------------------------------------------------------------------------------------
MAQ_DEFS = ('<defs><linearGradient id="maq_corpo" x1="0" y1="0" x2="0" y2="1"><stop offset="0" style="stop-color:#e7ebf1"></stop>'
            '<stop offset="1" style="stop-color:#98a2b0"></stop></linearGradient>'
            '<linearGradient id="maq_frente" x1="0" y1="0" x2="0" y2="1"><stop offset="0" style="stop-color:#4b5563"></stop>'
            '<stop offset="1" style="stop-color:#262c35"></stop></linearGradient></defs>')


def maquina(grande):
    """Duas gavetas de servidor empilhadas (x 5–43; y 7–23 e 25–41), com a frente escura, o led verde e as frestas; a de
    cima com o realce. Até 24 px, o contorno firme (2,4) do item 13 e só duas frestas."""
    traco = "1" if grande else "2.4"
    p_ = [MAQ_DEFS]
    for y in (7, 25):
        p_.append(f'<rect class="cont" filter="url(#icSombra)" x="5" y="{y}" width="38" height="16" rx="3.5" fill="url(#maq_corpo)" '
                  f'stroke="#000000" style="stroke-opacity:.35;stroke-width:{traco}"></rect>')
        p_.append(f'<rect x="9" y="{y + 4}" width="30" height="8" rx="2" fill="url(#maq_frente)"></rect>')
        p_.append(f'<circle cx="13.5" cy="{y + 8}" r="{1.8 if grande else 2.2}" fill="#3fd17f"></circle>')
        if grande:
            p_.append(f'<circle cx="13.5" cy="{y + 8}" r="3.6" fill="#3fd17f" style="fill-opacity:.25"></circle>')
        frestas = (21, 26, 31) if grande else (23, 30)
        d = "".join(f"M{x} {y + 6.2}v3.6" for x in frestas)
        p_.append(f'<path d="{d}" stroke="#ffffff" style="stroke-opacity:.35;stroke-width:{1.4 if grande else 2};stroke-linecap:round"></path>')
    p_.append('<rect x="5.5" y="7.5" width="37" height="4" rx="2" fill="url(#icRealce)"></rect>')
    if grande:
        p_.append('<rect x="5.5" y="25.5" width="37" height="3" rx="1.5" fill="url(#icRealce)" style="opacity:.6"></rect>')
        p_ += [f'<circle cx="{x}" cy="{y}" r=".9" fill="#5d6673"></circle>' for x in (7.6, 40.4) for y in (15, 33)]
    return p_


def molde(partes):
    return '<svg viewBox="0 0 48 48" width="{{z.s}}" height="{{z.s}}" aria-hidden="true">' + "".join(partes) + "</svg>"


NOVAS = {"a_maquina": molde(maquina(False)), "a_maquinaG": molde(maquina(True))}


def trocar(texto, antigo, novo):
    if texto.count(antigo) != 1:
        raise SystemExit(f"ssh: a biblioteca não tem uma vez só: {antigo[:70]!r}")
    return texto.replace(antigo, novo)


def biblioteca(t):
    """Põe a Máquina remota na seção Aplicativo da prancheta 9, como o icones_resumo.py pôs o disco e as linhas."""
    tem = [f for f in NOVAS if f'<sc-if value="{{{{z.{f}}}}}"' in t]
    if len(tem) == len(NOVAS):
        for f, m in NOVAS.items():
            if f'<sc-if value="{{{{z.{f}}}}}" hint-placeholder-val="{{{{ false }}}}">{m}</sc-if>' not in t:
                raise SystemExit(f"ssh: a forma {f} da biblioteca foi mudada à mão; acerte este gerador antes de rodar")
        return t, False
    if tem:
        raise SystemExit(f"ssh: a biblioteca tem só parte das formas novas: {', '.join(tem)}")
    i = t.index('<sc-if value="{{z.a_linhasG}}"')
    fim = t.index("</sc-if>", i) + len("</sc-if>")
    novo = "".join(f'\n<sc-if value="{{{{z.{k}}}}}" hint-placeholder-val="{{{{ false }}}}">{m}</sc-if>' for k, m in NOVAS.items())
    t = t[:fim] + novo + t[fim:]
    t = trocar(t, "a_linhas: false, a_linhasG: false, cor:", "a_linhas: false, a_linhasG: false, a_maquina: false, a_maquinaG: false, cor:")
    t = trocar(t, "linhas: { a_linhas: true } };", "linhas: { a_linhas: true }, maquina: { a_maquina: true } };")
    t = trocar(t, "        a_linhasG: b.a_linhas && s >= 32,\n",
               "        a_linhasG: b.a_linhas && s >= 32,\n        a_maquina: b.a_maquina && s < 32,\n        a_maquinaG: b.a_maquina && s >= 32,\n")
    t = trocar(t, "      item('linhas', 'Linhas', 'Visão geral > Linhas')\n    ];",
               "      item('linhas', 'Linhas', 'Visão geral > Linhas'),\n      item('maquina', 'Máquina remota', 'Explorador > raiz por SSH')\n    ];")
    return t, True


def maq(n=16):
    partes = maquina(n >= 32)
    return p.sk(f'<svg viewBox="0 0 48 48" width="{n}" height="{n}" aria-hidden="true">' + "".join(partes) + "</svg>", n)


# --- a folha desta prancheta ----------------------------------------------------------------------------------------------
PROPRIO = """
.dlg.ssh{width:480px;overflow:visible}
.par.dlgs{grid-template-columns:repeat(2,minmax(0,1fr))}
.mini-exp .aviso-j{max-width:100%;margin:0;justify-content:flex-start}
.mini-exp .aviso-j .corpo{flex:1}
.dlg .corpo-s{padding:16px 20px 18px;display:flex;flex-direction:column;gap:14px}
.frm{display:grid;grid-template-columns:max-content minmax(0,1fr);gap:10px 14px;align-items:center}
.frm>label{color:var(--t2);white-space:nowrap}
.frm .lin2{display:flex;gap:14px;align-items:center;min-width:0}
.frm .lin2 label{color:var(--t2);white-space:nowrap}
.frm .campo{width:100%;min-width:0}
.frm .campo.porta{width:auto;flex:none}
.frm .campo.porta input{width:6ch;flex:none}
.frm .dica-c{grid-column:2;margin-top:-6px;color:var(--t3);font-size:11.5px}
.campo.erro-c{border-color:var(--perigo);box-shadow:0 0 0 1px color-mix(in oklab,var(--perigo) 60%,transparent)}
.sug{position:absolute;padding:5px;border-radius:12px;min-width:100%;z-index:5}
.sug .mi{height:auto;min-height:40px;padding:5px 10px 5px 12px;align-items:center}
.sug .mi .d{display:flex;flex-direction:column;gap:1px;min-width:0}
.sug .mi .d small{color:var(--t3);font-size:11px}
.sug .cab-s{padding:6px 12px 4px;font-size:10.5px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;color:var(--t3)}
.rel{position:relative}
.dlg footer .esq-f{display:inline-flex;align-items:center;gap:6px;color:var(--t3);font-size:11.5px;min-width:0}
.digital{display:flex;flex-direction:column;gap:6px;padding:12px 14px;border-radius:10px;border:1px solid var(--fb);background:var(--p)}
.digital .rot-d{color:var(--t3);font-size:11px}
.digital .fp{display:block;font-size:12.5px;font-family:Consolas,"Cascadia Mono",monospace;letter-spacing:.02em;color:var(--t);overflow-wrap:anywhere;user-select:all}
.digital.velha .fp{color:var(--t2);text-decoration:line-through;text-decoration-color:color-mix(in oklab,var(--perigo) 70%,transparent)}
.digital.nova-p .fp{color:var(--perigo)}
.dados-maq{display:grid;grid-template-columns:max-content minmax(0,1fr);gap:6px 14px;color:var(--t2)}
.dados-maq dt{color:var(--t3)}
.dados-maq dd{margin:0;color:var(--t);overflow-wrap:anywhere}
.op-m{display:flex;align-items:flex-start;gap:8px;color:var(--t)}
.op-m input{margin-top:2px}
.op-m small{display:block;color:var(--t3);font-size:11.5px}
.dlg header .sk{margin-right:10px}
.no .remota{font-weight:600;color:var(--t)}
.no .host{color:var(--t3);font-weight:400}
.no .lat{margin-left:auto;display:inline-flex;align-items:center;gap:5px;color:var(--t3);font-size:11px}
.ponto{width:7px;height:7px;border-radius:50%;background:var(--ok);flex:none}
.ponto.caiu{background:var(--t3)}
.no.raiz-off{color:var(--t3)}
.no.raiz-off .sk{opacity:.55;filter:grayscale(1)}
.dica-m{position:absolute;width:auto;max-width:300px;padding:8px 12px;border-radius:8px;font-size:11.5px;line-height:18px;color:var(--t);white-space:nowrap;z-index:6}
.dica-m .t3{color:var(--t3)}
.so-leit{display:inline-flex;align-items:center;gap:6px;height:22px;padding:0 10px 0 6px;border-radius:999px;background:var(--h);border:1px solid var(--vidrob);color:var(--t2);font-size:11px;font-weight:600;white-space:nowrap}
.faixa .ctx{color:var(--t2);min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.trazer{flex:1;min-height:0;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:12px;padding:24px;text-align:center;color:var(--t2);background:var(--c)}
.trazer .ic.gira{width:22px;height:22px;color:var(--acct)}
.trazer .barra{width:280px}
.trazer>*{flex:none}
.trazer .num{color:var(--t3);font-size:11.5px}
.trazer .acoes{display:flex;gap:8px}
.lista-t td.mono,.det dd.mono{font-family:Consolas,"Cascadia Mono",monospace;font-size:11.5px}
.empilha{display:flex;flex-direction:column;gap:14px}
.par{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:16px;align-items:start}
.arvore{padding:6px 0 8px}
.mini-exp{width:300px;flex:none;border-radius:10px;display:flex;flex-direction:column;overflow:visible;position:relative}
"""

S = '<span class="sep"></span>'
CADEADO = p.forma("st_bloqueado", 16).replace('<rect class="cont" filter="url(#icSombra)"', '<rect filter="url(#icSombra)"')


def chip_leitura():
    return f'<span class="so-leit">{p.sk(CADEADO)}Somente leitura</span>'


def dlg(icone, titulo, corpo, rodape, classe="dlg flut ssh", sub=""):
    s = f'<span style="color:var(--t3);margin-left:8px">{sub}</span>' if sub else ""
    return (f'<section class="{classe}" role="dialog" aria-label="{titulo}"><header>{icone}<h2>{titulo}</h2>{s}<span style="flex:1"></span>'
            f'<button class="ib" aria-label="Fechar" title="Fechar (Esc)">{p.FECHAR_DLG}</button></header>'
            f'<div class="corpo-s">{corpo}</div><footer>{rodape}</footer></section>')


def fundo(dialogo, extra=""):
    return f'<div class="fundo-dlg"><div class="esq" aria-hidden="true"></div><div class="veu" aria-hidden="true"></div>{dialogo}{extra}</div>'


def entrada(valor, aria, classe="campo", tipo="text", placeholder="", estilo=""):
    ph = f' placeholder="{placeholder}"' if placeholder else ""
    v = f' value="{valor}"' if valor else ""
    st = f' style="{estilo}"' if estilo else ""
    return f'<label class="{classe}"{st}><input type="{tipo}"{v}{ph} aria-label="{aria}"></label>'


def no(rotulo, nivel, icone="", seta=None, classe="no", extra="", dica=""):
    s = (p.SETA_ABERTA if seta == "aberta" else p.SETA_FECHADA) if seta else '<span class="vz"></span>'
    t = f' title="{dica}"' if dica else ""
    return f'<a class="{classe}" href="#" style="padding-left:{8 + 14 * nivel}px"{t}>{s}{icone}<span>{rotulo}</span>{extra}</a>'


# --- (a) o menu Arquivo ------------------------------------------------------------------------------------------------
def cena_menu():
    menu = ('<div role="menu" aria-label="Arquivo" class="flut menu-ctx" style="position:relative;width:auto;min-width:300px;align-self:flex-start">'
            + p.mi("Abrir pasta", "Ctrl+Shift+O", icone=p.sil("Abrir pasta")) + p.mi("Abrir arquivo", "Ctrl+O", icone=p.sil("Abrir arquivo"))
            + p.mi("Conectar por SSH", "Ctrl+Shift+K", classe="mi on") + p.mi("Abertos recentemente", "›") + p.SEP
            + p.mi("Novo banco") + p.mi("Salvar", "Ctrl+S", icone=p.sil("Salvar"), classe="mi off", desligado=True) + p.SEP
            + p.mi("Preferências", "Ctrl+,") + p.SEP + p.mi("Sair", "Alt+F4") + "</div>")
    vazio = ('<aside class="vidro mini-exp" aria-label="Explorador" style="width:258px"><div style="padding:22px 16px;display:flex;'
             'flex-direction:column;align-items:center;gap:10px;text-align:center;color:var(--t2)">Nenhuma pasta aberta.'
             '<div style="display:flex;flex-wrap:wrap;gap:6px;justify-content:center"><button class="btn pri pq">Abrir pasta</button>'
             '<button class="btn pq">Conectar por SSH</button></div></div></aside>')
    menu = (f'<div class="etapa"><span class="tit">1 · O menu Arquivo</span>{menu}</div>'
            f'<div class="etapa" style="margin-top:8px"><span class="tit">2 · O Explorador sem pasta</span>{vazio}</div>')
    return p.estado(
        "Arquivo › Conectar por SSH",
        "Logo depois de Abrir pasta e Abrir arquivo, porque é a terceira maneira de abrir. Sem reticências (a regra dos menus vale "
        "também aqui, embora o plano escreva \"Conectar por SSH…\") e sem silhueta: não há botão dele na barra (U11). Ctrl+Shift+K, "
        "livre no app. O Início ganha o mesmo verbo, como um link ao lado de Abrir pasta, e o Explorador sem pasta oferece os dois. "
        "Na 0.3.0 não há conexões recentes: o submenu Abertos recentemente continua só com pastas e arquivos locais.",
        menu)


# --- (b) a janela Conectar por SSH ---------------------------------------------------------------------------------------
HOSTS = [("servidor", "isaias@10.0.4.12 · porta 22"), ("gpu-lab", "isaias@gpu-lab.fgv.br · porta 2222"),
         ("vm-teste", "dev@192.168.56.10 · porta 22")]


def form_conexao(destino="servidor", usuario="isaias", porta="22", pasta="/srv/etl", lista=False, extra="", desligar=False):
    dis = ' disabled=""' if desligar else ""
    sug = ""
    if lista:
        itens = "".join(f'<a class="mi{" on" if i == 0 else ""}" role="option" href="#">{maq()}<span class="d"><span>{h}</span><small>{d}</small></span></a>'
                        for i, (h, d) in enumerate(HOSTS))
        sug = (f'<div role="listbox" class="flut sug" style="left:0;top:32px">'
               f'<div class="cab-s">Do ~/.ssh/config</div>{itens}</div>')
    campo_dest = (f'<div class="rel"><label class="campo"><input value="{destino}" aria-label="Destino" aria-autocomplete="list"{dis}>'
                  f'<span style="flex:1"></span>{p.SETA_CAMPO}</label>{sug}</div>')
    porta_c = f'<label class="campo porta"><input value="{porta}" inputmode="numeric" aria-label="Porta"{dis}></label>'
    return ('<div class="frm">'
            f'<label>Destino</label>{campo_dest}'
            '<span class="dica-c">isaias@servidor, servidor:2222 ou um Host do ~/.ssh/config</span>'
            f'<label>Usuário</label><div class="lin2">{entrada(usuario, "Usuário", estilo="flex:1")}<label>Porta</label>{porta_c}</div>'
            f'<label>Pasta inicial</label>{entrada(pasta, "Pasta inicial", placeholder="a pasta de casa")}'
            f'{extra}</div>')


def rodape_dlg(acao="Conectar", ocupado=False, esq=""):
    e = f'<span class="esq-f">{esq}</span>' if esq else ""
    if ocupado:
        return (f'{e}<span style="flex:1"></span><button class="btn sutil" title="Esc">Cancelar</button>'
                f'<button class="btn pri" aria-disabled="true" style="opacity:.75;cursor:progress">{p.GIRA}Conectando</button>')
    return f'{e}<span style="flex:1"></span><button class="btn sutil" title="Esc">Cancelar</button><button class="btn pri">{acao}</button>'


CHAVES = ('<span title="O DataBox usa o que o ssh do terminal usa: o ssh-agent (ou o agente do OpenSSH do Windows, ou o Pageant) e as '
          'chaves ~/.ssh/id_*. A senha só aparece se a máquina pedir; ela não fica guardada.">Entra com as suas chaves e o agente</span>')


def cena_conectar():
    d1 = dlg(maq(20), "Conectar por SSH", form_conexao(destino="serv", usuario="isaias", porta="22", pasta="", lista=True), rodape_dlg(esq=CHAVES))
    d2 = dlg(maq(20), "Conectar por SSH", form_conexao(desligar=True)
             + '<div class="nota-j" style="margin:0">' + p.sk(p.GIRA.replace('class="ic gira"', 'class="ic gira" style="width:16px;height:16px"'))
             + '<span>Conectando a servidor (10.0.4.12, porta 22)…</span></div>', rodape_dlg(ocupado=True))
    senha = ('<label>Senha</label>' + entrada("••••••••••", "Senha de isaias@servidor", tipo="password")
             + '<span class="dica-c">servidor pediu a senha de isaias. Ela vale só para esta conexão.</span>')
    d3 = dlg(maq(20), "Conectar por SSH", form_conexao(extra=senha), rodape_dlg("Entrar"))
    frase = ('<label>Frase da chave</label>' + entrada("", "Frase da chave id_ed25519", tipo="password", classe="campo erro-c", placeholder="")
             + '<span class="dica-c" style="color:var(--perigo)">A frase não abriu ~/.ssh/id_ed25519. Tente de novo.</span>')
    d4 = dlg(maq(20), "Conectar por SSH", form_conexao(destino="gpu-lab", porta="2222", pasta="~/experimentos", extra=frase), rodape_dlg("Entrar"))
    erro = p.aviso("Não deu para conectar a gpu-lab: a máquina não respondeu em 15 s. Confira o nome e a porta, ou se a VPN está ligada.",
                   perigo=True)
    d5 = dlg(maq(20), "Conectar por SSH", form_conexao(destino="gpu-lab", porta="2222", pasta="~/experimentos") + erro, rodape_dlg("Tentar de novo"))
    palco = ('<div class="par dlgs">'
             f'<div class="etapa"><span class="tit">1 · O destino, com os Host do ~/.ssh/config</span>{fundo(d1)}</div>'
             f'<div class="etapa"><span class="tit">2 · Conectando</span>{fundo(d2)}</div>'
             f'<div class="etapa"><span class="tit">3 · A senha, só quando a máquina pede</span>{fundo(d3)}</div>'
             f'<div class="etapa"><span class="tit">4 · A frase da chave, recusada</span>{fundo(d4)}</div>'
             f'<div class="etapa"><span class="tit">5 · A máquina não responde</span>{fundo(d5)}</div>'
             "</div>")
    return p.estado(
        "A janela Conectar por SSH",
        "Uma janela do app (vidro, 480 px), modal, com o foco no Destino. O Destino é um campo com sugestões: digitar filtra os Host "
        "do ~/.ssh/config (o nome e, embaixo, usuário, endereço e porta que o config resolve); escolher um preenche Usuário, Porta "
        "e o que mais o config disser (IdentityFile, ProxyJump). Também aceita isaias@servidor e servidor:2222, que se repartem "
        "nos campos. A coluna dos rótulos mede o rótulo mais longo; a Porta mede 6 algarismos; nada tem largura pelo português. "
        "Nenhum campo de senha de saída: o DataBox entra com as chaves e o agente, e só se a máquina pedir aparece a Senha (ou a "
        "Frase da chave, com o nome do arquivo), sem \"Lembrar\": na 0.3.0 nada é guardado, e a dica diz isso. Conectar vira "
        "\"Entrar\" quando há senha a mandar. Conectando, os campos desligam, o principal mostra o arco e Cancelar (Esc) desiste. "
        "Os erros vão na caixa de aviso, com a causa e o que conferir; o erro da senha ou da frase fica no campo.",
        palco, largo=True)


# --- (c) a impressão digital da chave da máquina ------------------------------------------------------------------------------
FP_NOVA = "SHA256:q8Wm3n4zR1yL0cJx9TfKpV2bA6sDhE7uGiO5wNkYtXc"
FP_VELHA = "SHA256:Zt4P1yQ8mBvN6xKc2LwR0aJ7sGh3TfUe9dYo5iVnHqA"


def cena_digital():
    nova = (
        '<p style="margin:0;color:var(--t2);text-wrap:pretty">É a primeira vez que o DataBox se conecta a <strong style="color:var(--t)">servidor</strong>. '
        "Confira a impressão digital com quem cuida da máquina (ou com <span class=\"mono\">ssh-keygen -lf</span> nela) antes de continuar.</p>"
        '<dl class="dados-maq"><dt>Máquina</dt><dd>servidor · 10.0.4.12, porta 22</dd><dt>Chave</dt><dd>ED25519</dd></dl>'
        f'<div class="digital"><span class="rot-d">Impressão digital (SHA-256)</span><span class="fp">{FP_NOVA}</span>'
        f'<div><button class="btn sutil pq">{p.COPIAR}Copiar</button></div></div>'
        '<label class="op-m"><input type="checkbox" aria-label="Lembrar esta máquina"><span>Lembrar esta máquina'
        '<small>Grava a chave no ~/.ssh/known_hosts, como o ssh do terminal.</small></span></label>')
    d1 = dlg(p.sk(p.forma("st_info", 20), 20), "Máquina nova", nova,
             '<span style="flex:1"></span><button class="btn sutil" title="Esc">Cancelar</button><button class="btn pri">Confiar e conectar</button>')
    mudou = (
        p.aviso("A chave que servidor mostrou agora não é a que o seu known_hosts guarda. Pode ser uma reinstalação da máquina, ou "
                "alguém no meio do caminho. O DataBox não se conectou.", perigo=True)
        + '<dl class="dados-maq"><dt>Máquina</dt><dd>servidor · 10.0.4.12, porta 22</dd><dt>Guardada em</dt><dd>~/.ssh/known_hosts, linha 14</dd></dl>'
        f'<div class="digital velha"><span class="rot-d">A conhecida · ED25519</span><span class="fp">{FP_VELHA}</span></div>'
        f'<div class="digital nova-p"><span class="rot-d">A de agora · ED25519</span><span class="fp">{FP_NOVA}</span></div>'
        '<p style="margin:0;color:var(--t2);text-wrap:pretty">Se a troca for esperada, tire a chave antiga com '
        '<span class="mono">ssh-keygen -R servidor</span> e conecte de novo.</p>')
    d2 = dlg(p.sk(p.ERRO, 20), "A chave de servidor mudou", mudou,
             f'<button class="btn sutil pq">{p.COPIAR}Copiar os detalhes</button><span style="flex:1"></span><button class="btn pri">Fechar</button>')
    palco = ('<div class="par">'
             f'<div class="etapa"><span class="tit">1 · Máquina nova</span>{fundo(d1)}</div>'
             f'<div class="etapa"><span class="tit">2 · Chave mudada: recusada</span>{fundo(d2)}</div></div>')
    return p.estado(
        "A impressão digital da chave da máquina",
        "Depois do Conectar e antes de qualquer senha, a chave da máquina é conferida contra o ~/.ssh/known_hosts da pessoa. Igual: "
        "segue sem janela. Máquina nova: esta janela, com a impressão SHA-256 em mono (seleciona inteira com um clique e tem "
        "Copiar), o tipo da chave e a máquina; o principal é \"Confiar e conectar\", o Esc e o Cancelar desistem, e \"Lembrar esta "
        "máquina\" (desmarcado) grava no known_hosts; sem ele, a próxima conexão pergunta de novo. Chave mudada: a conexão já foi "
        "recusada quando a janela abre; ela não tem botão de aceitar, mostra as duas impressões (a guardada riscada, a nova em "
        "--perigo), onde a antiga está e o comando que a tira. É a única saída: o DataBox nunca troca a chave sozinho. O log diz "
        "a máquina e a impressão, nunca a credencial.",
        palco, largo=True)


# --- (d) o Explorador com a raiz remota e a aba remota --------------------------------------------------------------------
LINHAS_CSV = [("2026-10-26 03:00:12", "carga_clientes", "ok", "60", "1,8"), ("2026-10-26 03:00:14", "carga_pedidos", "ok", "1.240", "3,1"),
              ("2026-10-26 03:00:19", "carga_itens", "ok", "4.471", "6,4"), ("2026-10-26 03:00:27", "agrega_vendas", "ok", "236", "2,2"),
              ("2026-10-26 03:00:31", "exporta_bi", "aviso", "236", "0,9"), ("2026-10-26 03:00:33", "limpa_temp", "ok", "0", "0,1"),
              ("2026-10-26 03:01:02", "carga_eventos", "erro", "0", "29,4"), ("2026-10-26 03:01:40", "carga_eventos", "ok", "88.410", "37,8"),
              ("2026-10-26 03:02:05", "valida_chaves", "ok", "5.772", "4,0"), ("2026-10-26 03:02:11", "notifica", "ok", "1", "0,3"),
              ("2026-10-26 03:02:12", "fim", "ok", "—", "0,0")]


def arvore_remota(sel="saida.csv", dica=True):
    lat = '<span class="lat" title="Conectada · 38 ms"><span class="ponto"></span></span>'
    raiz = no('<span class="remota">isaias@servidor</span><span class="host">:/srv/etl</span>', 0, maq(), "aberta", extra=lat,
              dica="isaias@servidor:/srv/etl")
    itens = [no("brutos", 1, p.sk(p.PASTA16), "fechada"), no("logs", 1, p.sk(p.PASTA16), "fechada"),
             no("experimentos.db", 1, p.sk(p.BANCO16), "fechada"),
             no("saida.csv", 1, p.folha("CSV"), classe="no on" if sel == "saida.csv" else "no"),
             no("eventos_2026.csv", 1, p.folha("CSV"), classe="no on" if sel == "eventos_2026.csv" else "no"),
             no("config.json", 1, p.folha("JSON")), no("leia-me.txt", 1, p.folha("TXT"))]
    local = no("dados", 0, p.sk(p.PASTA16), "fechada", dica="C:\\dados")
    return local + raiz + "".join(itens)


DICA_MAQ = ('<div role="tooltip" class="flut dica-m" style="left:236px;top:40px"><strong>servidor</strong> · isaias@10.0.4.12, porta 22<br>'
            '<span class="t3">Conectado há 4 min · latência 38 ms · chave ED25519</span><br>'
            '<span class="t3">/srv/etl · 7 itens · só leitura nesta versão</span></div>')


def doca_explorador(conteudo, largura=258, extra=""):
    acoes = p.botao("Abrir pasta", classe="ib pq") + p.botao("Recolher tudo", classe="ib pq") + p.botao("Destacar em janela", classe="ib pq")
    abas = (f'<div role="tablist" aria-label="Painéis à esquerda" class="abas-doca"><div class="aba so-ic on" role="presentation">'
            f'<a href="#" role="tab" aria-selected="true" aria-label="Explorador" title="Explorador">{p.sk(p.icone_app("explorador"))}</a></div>'
            f'<div class="aba so-ic" role="presentation"><a href="#" role="tab" aria-selected="false" aria-label="Histórico" title="Histórico">{p.sk(p.icone_app("historico"))}</a></div>'
            f'<span style="flex:1"></span><span class="sep-doca" aria-hidden="true"></span>{acoes}</div>')
    return (f'<div class="doca" style="width:{largura}px;position:relative">{abas}<aside class="vidro" aria-label="Explorador" style="border-radius:10px;overflow:visible">'
            f'<div class="arvore">{conteudo}</div></aside>{extra}</div>')


def grade_csv():
    th = (f'<thead><tr><th class="num" style="width:48px;color:var(--t3)">#</th><th style="width:170px">{p.P_DATA}quando</th>'
          f'<th style="width:150px">{p.P_TXT}etapa</th><th style="width:90px">{p.P_TXT}situacao</th>'
          f'<th class="num" style="width:96px">{p.P_INT}linhas</th><th class="num" style="width:90px">{p.P_DEC}segundos</th><th></th></tr></thead>')
    corpo = "".join(f'<tr><td class="lin">{i + 1}</td><td>{q}</td><td>{e}</td><td>{s}</td><td class="num">{n}</td><td class="num">{t}</td><td></td></tr>'
                    for i, (q, e, s, n, t) in enumerate(LINHAS_CSV))
    return '<table class="grade">' + th + "<tbody>" + corpo + "</tbody></table>"


def cena_explorador():
    barra = p.barra(p.botao("Voltar"), p.botao("Avançar", desligado=True), S, p.botao("Abrir pasta"), p.botao("Abrir arquivo"),
                    p.botao("Salvar", dica="Somente leitura: saida.csv é uma cópia de servidor", desligado=True), S,
                    p.botao("Nova linha", desligado=True), p.botao("Nova coluna", desligado=True), p.botao("Duplicar", desligado=True),
                    p.botao("Excluir", desligado=True), p.botao("Desfazer", desligado=True), p.botao("Refazer", desligado=True), S,
                    p.botao("Filtrar"), p.botao("Atualizar", dica="Trazer saida.csv de servidor de novo (F5)"), S,
                    p.botao("Exportar", dica="Exportar saida.csv"))
    abas = p.abas(p.aba(p.sk(p.TABELA16), "clientes", dica="C:\\dados\\exemplo.db › clientes"),
                  p.aba(p.folha("CSV"), "saida.csv", on=True, sub=" · servidor",
                        dica="isaias@servidor:/srv/etl/saida.csv (cópia local de 26/10/2026 14:32)"))
    faixa = (f'<div class="faixa">{chip_leitura()}<span class="ctx">Cópia de servidor, trazida às 14:32 · lá, modificado às 03:02</span>'
             '<span style="flex:1"></span><button class="btn sutil pq">Trazer de novo</button></div>')
    doc = ('<div class="doc" style="flex:1">' + faixa + '<div style="flex:1;min-height:0;overflow:hidden;background:var(--c)">' + grade_csv() + "</div>"
           '<div class="rodape"><span>Linhas 1–11 de 11 · 1,8 KB</span><span style="color:var(--t3)">· Vírgula · UTF-8 · LF</span></div></div>')
    prop = ('<aside class="vidro" aria-label="Propriedades" style="width:272px;flex:none;border-radius:10px;display:flex;flex-direction:column;overflow:hidden">'
            '<div class="cab"><span class="tit">Propriedades</span></div>'
            '<div class="sec">Arquivo</div><dl class="det">'
            '<dt>Máquina</dt><dd>servidor<br><span style="color:var(--t3)">isaias@10.0.4.12:22</span></dd>'
            '<dt>Caminho</dt><dd>/srv/etl/saida.csv</dd><dt>Tamanho</dt><dd>1,8 KB</dd>'
            '<dt>Modificado lá</dt><dd>26/10/2026 03:02</dd><dt>Trazido em</dt><dd>26/10/2026 14:32</dd>'
            '<dt>Cópia local</dt><dd title="~/.local/state/DataBox/remoto/servidor/srv/etl/saida.csv">…/srv/etl/saida.csv</dd></dl>'
            f'<div style="padding:0 8px 10px"><button class="btn sutil lista">{p.COPIAR}Copiar caminho</button></div>'
            '<div class="sec">Formato</div><dl class="det"><dt>Separador</dt><dd>Vírgula</dd><dt>Codificação</dt><dd>UTF-8</dd>'
            '<dt>Fim de linha</dt><dd>LF</dd></dl></aside>')
    status = ('<footer class="vidro" style="height:26px;flex:none;border-radius:10px;display:flex;align-items:center;gap:14px;padding:0 14px;font-size:11px;color:var(--t2)">'
              '<span style="display:inline-flex;align-items:center;gap:6px"><span class="ponto"></span>isaias@servidor · 38 ms</span>'
              '<span>Somente leitura</span><span style="flex:1"></span><span>CSV</span><span>UTF-8</span></footer>')
    janela = (barra + '<div style="display:flex;gap:8px;height:520px;flex:none">'
              + doca_explorador(arvore_remota(), extra=DICA_MAQ)
              + '<section style="flex:1;min-width:0;display:flex;flex-direction:column">' + abas + doc + "</section>" + prop + "</div>" + status)
    return p.estado(
        "O Explorador com a raiz remota e a aba saida.csv · servidor",
        "A pasta remota entra como mais uma raiz, abaixo das locais, com o sólido novo da Máquina remota, o usuário e a máquina "
        "em peso 600 e o caminho em --t3 (isaias@servidor:/srv/etl). Na ponta, o ponto verde da conexão viva; a "
        "dica diz a máquina, o endereço, há quanto tempo está conectada, a latência e a chave. Dentro, o mesmo Explorador: pastas, "
        "bancos pelo cabeçalho, CSV, Excel, JSON e texto, os mesmos limites (500 itens, 5 s), lidos pelo SFTP. Um clique abre o "
        "arquivo com o editor de sempre, sobre a cópia no cache; a aba diz o nome e, em --t3, a máquina (\"saida.csv · servidor\"), "
        "e a dica tem o endereço inteiro e a hora da cópia. Na 0.3.0 tudo que é remoto abre só para leitura: a faixa mostra o chip "
        "Somente leitura e de onde veio a cópia, com \"Trazer de novo\" (o Atualizar, F5); Salvar e a edição desligam, com o motivo na "
        "dica. Filtrar, ordenar, Exportar, Perfil, Salvar como e gráficos funcionam, porque leem o cache. As Propriedades contam a "
        "máquina, o caminho lá, as datas de lá e da cópia, e a cópia local. A barra de status mostra a conexão.",
        janela, largo=True)


# --- (e) trazer o arquivo -------------------------------------------------------------------------------------------------
def doc_trazer(nome, corpo, rodape):
    abas = p.abas(p.aba(p.folha("CSV"), nome, on=True, sub=" · servidor", dica=f"isaias@servidor:/srv/etl/{nome}"))
    return (f'<div style="display:flex;flex-direction:column">{abas}<div class="doc" style="height:212px">'
            f'<div class="trazer">{corpo}</div><div class="rodape"><span>{rodape}</span></div></div></div>')


def cena_trazer():
    t1 = doc_trazer("eventos_2026.csv", p.GIRA + '<span>Trazendo eventos_2026.csv de servidor</span>'
                    '<div class="barra" role="progressbar" aria-valuenow="43" aria-valuemin="0" aria-valuemax="100"><div style="width:43%"></div></div>'
                    '<span class="num">412 de 958 MB · 11,2 MB/s · cerca de 49 s</span><button class="btn risco pq">Interromper</button>',
                    "Trazendo · 37 s")
    t2 = doc_trazer("dump_completo.csv", p.aviso("dump_completo.csv tem 3,4 GB. O arquivo vem inteiro para o computador e ocupa 3,4 GB "
                                                  "no disco; a 11 MB/s, leva uns 5 min.", acoes='<button class="btn pri pq">Trazer mesmo assim</button>'
                                                  '<button class="btn sutil pq">Cancelar</button>'), "3,4 GB em servidor")
    t3 = doc_trazer("eventos_2026.csv", '<span>Interrompido. Nada ficou no cache.</span><div class="acoes"><button class="btn pq">Trazer de novo</button></div>',
                    "Interrompido em 412 de 958 MB")
    palco = (f'<div class="empilha"><div class="etapa"><span class="tit">1 · Trazendo</span>{t1}</div>'
             f'<div class="etapa"><span class="tit">2 · Acima de 1 GB, pergunta antes</span>{t2}</div>'
             f'<div class="etapa"><span class="tit">3 · Interrompido</span>{t3}</div></div>')
    return p.estado(
        "Trazer o arquivo, com Interromper",
        "A aba abre na hora e mostra o andamento enquanto o arquivo vem para o cache: o arco, o nome e a máquina, a barra, os MB, a "
        "velocidade e o tempo que falta, e o Interromper (botão de risco, Esc), como a Consulta executando. Só aparece depois de "
        "400 ms; um arquivo pequeno abre direto. Acima de 1 GB, a aba pergunta antes de trazer. Interromper apaga o pedaço trazido "
        "(o cache só guarda arquivo inteiro) e oferece Trazer de novo. Abrir de novo um arquivo que não mudou lá (mesmo tamanho e "
        "data) usa o cache sem trazer nada.",
        palco)


# --- (f) o banco remoto, cópia só leitura -----------------------------------------------------------------------------------
def cena_banco():
    banco_ro = p.sk(p.icone_app("banco_so_leitura"))
    arv = (no('<span class="remota">isaias@servidor</span><span class="host">:/srv/etl</span>', 0, maq(), "aberta",
              extra='<span class="lat" title="Conectada · 38 ms"><span class="ponto"></span></span>')
           + no("experimentos.db", 1, banco_ro, "aberta", classe="no on") + no("Tabelas", 2, seta="aberta", extra='<span class="q">3</span>')
           + no("execucoes", 3, p.sk(p.TABELA16), extra='<span class="q">1.204</span>')
           + no("metricas", 3, p.sk(p.TABELA16), extra='<span class="q">88.410</span>')
           + no("parametros", 3, p.sk(p.TABELA16), extra='<span class="q">96</span>')
           + no("Visões", 2, seta="fechada", extra='<span class="q">1</span>'))
    exp = f'<aside class="vidro mini-exp" aria-label="Explorador"><div class="arvore">{arv}</div></aside>'
    abas = p.abas(p.aba(banco_ro, "experimentos.db", on=True, sub=" · servidor", dica="isaias@servidor:/srv/etl/experimentos.db (cópia local de 14:32)"))
    faixa = (f'<div class="faixa">{chip_leitura()}'
             '<span style="flex:1"></span><button class="btn sutil pq">Trazer de novo</button></div>')
    tabela = ('<div style="padding:14px"><table class="lista-t" style="border:1px solid var(--b2);border-radius:10px"><thead><tr><th>Nome</th>'
              '<th class="num">Linhas</th><th class="num">Colunas</th></tr></thead><tbody>'
              '<tr><td>execucoes</td><td class="num">1.204</td><td class="num">9</td></tr>'
              '<tr><td>metricas</td><td class="num">88.410</td><td class="num">5</td></tr>'
              '<tr><td>parametros</td><td class="num">96</td><td class="num">4</td></tr></tbody></table></div>')
    doc = (f'<div style="flex:1;min-width:0;display:flex;flex-direction:column">{abas}<div class="doc">{faixa}{tabela}'
           '<div class="rodape"><span>3 tabelas · 1 visão · 14,2 MB · cópia de 26/10 14:32</span></div></div></div>')
    palco = f'<div style="display:flex;gap:10px;align-items:flex-start">{exp}{doc}</div>'
    return p.estado(
        "O banco SQLite remoto: uma cópia só leitura",
        "O SQLite não abre pela rede (o travamento dele não funciona sobre SFTP), então o banco remoto vem inteiro para o cache e "
        "abre como o banco só leitura de hoje: o cilindro cinza com o cadeado, na árvore e na aba, Dados, Consulta (só SELECT), "
        "estrutura e Exportar. A faixa diz que é uma cópia e de quando, com Trazer de novo. Editar, \"Enviar ao servidor\" e a "
        "pergunta ao fechar são da 0.4 e não aparecem aqui, nem desligados. Um banco com -wal ou -journal ao lado, lá, é trazido "
        "assim mesmo, mas a faixa avisa que pode não ter as gravações mais recentes.",
        palco)


# --- (g) o menu da raiz e a conexão que cai -------------------------------------------------------------------------------
def cena_conexao():
    menu = ('<div role="menu" aria-label="isaias@servidor" class="flut menu-ctx" style="left:200px;top:30px;width:auto;min-width:250px">'
            + p.mi("Atualizar", "F5", icone=p.sil("Atualizar")) + p.mi("Copiar caminho", "Ctrl+Shift+C", icone=p.COPIAR)
            + p.SEP + p.mi("Desconectar") + "</div>")
    arv1 = no('<span class="remota">isaias@servidor</span><span class="host">:/srv/etl</span>', 0, maq(), "aberta",
              classe="no on", extra='<span class="lat" title="Conectada · 38 ms"><span class="ponto"></span></span>')
    arv1 += no("brutos", 1, p.sk(p.PASTA16), "fechada") + no("logs", 1, p.sk(p.PASTA16), "fechada")
    exp1 = f'<aside class="vidro mini-exp" aria-label="Explorador" style="height:150px"><div class="arvore">{arv1}</div></aside>{menu}'
    arv2 = no('<span>isaias@servidor</span><span class="host">:/srv/etl</span>', 0, maq(), "fechada", classe="no raiz-off",
              extra='<span class="lat"><span class="ponto caiu"></span>caiu</span>')
    nota = ('<div style="padding:8px 10px 4px">' + p.aviso("A conexão com servidor caiu às 14:51. As abas abertas continuam, com a cópia "
                                                               "que já estava aqui.", acoes='<button class="btn pq">Reconectar</button>') + "</div>")
    exp2 = f'<aside class="vidro mini-exp" aria-label="Explorador"><div class="arvore">{arv2}</div>{nota}</aside>'
    palco = ('<div class="empilha">'
             f'<div class="etapa"><span class="tit">1 · O menu da raiz remota</span><div style="position:relative;height:160px">{exp1}</div></div>'
             f'<div class="etapa"><span class="tit">2 · A conexão caiu</span>{exp2}</div></div>')
    return p.estado(
        "O menu da raiz remota e a conexão que cai",
        "O clique direito na raiz remota: Atualizar (lê a pasta de novo), Copiar caminho (isaias@servidor:/srv/etl, o formato do scp) "
        "e Desconectar, que fecha a sessão e tira a raiz; as abas remotas ficam abertas, só leitura, sobre a cópia. Fechar o app "
        "desconecta tudo, e a raiz não volta na próxima abertura (isso é da 0.4). Quando a conexão cai, a raiz fica em --t3, o "
        "ponto cinza diz \"caiu\" e a caixa de aviso oferece Reconectar, que pede a senha de novo se for o caso. Nada se perde: o que "
        "estava aberto é a cópia local.",
        palco)


# --- a página -------------------------------------------------------------------------------------------------------------
icones_pg = p.ler("Icones")
novo_icones, mudou_icones = biblioteca(icones_pg)
cenas = [cena_menu(), cena_trazer(), cena_conectar(), cena_digital(), cena_explorador(), cena_banco(), cena_conexao()]
quadro = '<div class="quadro">' + "".join(cenas) + "</div>"
texto = p.pagina("SSH na 0.3.0", MARCA, PROPRIO, ALTURA, quadro)
p.conferir_refs(texto, ARQUIVO)
if "--medir" in sys.argv:
    print(texto)
    sys.exit(0)
if mudou_icones:
    (p.PASTA / "Icones.dc.html").write_text(novo_icones, encoding="utf-8", newline="\n")
    print("Icones.dc.html: a Máquina remota na seção Aplicativo")
p.gravar(ARQUIVO, texto, MARCA, {"expand": "fill", "h": ALTURA, "page": "futuro", "title": TITULO, "w": 1440, **LUGAR})
