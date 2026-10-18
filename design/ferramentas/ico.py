"""Ícone do executável: monta crates/editor/recursos/databox.ico com os PNG do logo de crates/interface/assets/logo
(os mesmos que o app embute, rasterizados de design/logo/databox.svg). É sempre o logo completo (databox-*), em
todos os tamanhos, de 16 a 256 px (decisão de 17/10/2026: não há versão simplificada). Cada imagem vai inteira,
em PNG, numa entrada do ICONDIR: o Windows lê assim desde o Vista e o windres embute a entrada como veio.
Só biblioteca padrão. Rode de novo quando o logo mudar; com --conferir, só lista o que o .ico atual tem."""
import struct
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
LOGO = RAIZ / "crates" / "interface" / "assets" / "logo"
DESTINO = RAIZ / "crates" / "editor" / "recursos" / "databox.ico"
sys.stdout.reconfigure(encoding="utf-8")

# (lado em pixels, arquivo), do menor para o maior
IMAGENS = [(lado, f"databox-{lado}.png") for lado in (16, 20, 24, 32, 48, 64, 96, 128, 256)]
ASSINATURA_PNG = b"\x89PNG\r\n\x1a\n"


def lado_do_png(dados, nome):
    """O lado de um PNG quadrado RGBA de 8 bits (o que o Windows espera numa entrada PNG de ícone)."""
    if dados[:8] != ASSINATURA_PNG or dados[12:16] != b"IHDR":
        raise SystemExit(f"{nome} não é um PNG.")
    largura, altura = struct.unpack(">II", dados[16:24])
    profundidade, tipo_de_cor = dados[24], dados[25]
    if largura != altura:
        raise SystemExit(f"{nome} não é quadrado ({largura}×{altura}).")
    if (profundidade, tipo_de_cor) != (8, 6):
        raise SystemExit(f"{nome} não é RGBA de 8 bits (profundidade {profundidade}, tipo de cor {tipo_de_cor}).")
    return largura


def montar(imagens):
    """O .ico com uma entrada PNG por imagem: [(lado, bytes do PNG)]."""
    cabecalho = struct.pack("<HHH", 0, 1, len(imagens))  # reservado, tipo 1 = ícone, quantas
    inicio = 6 + 16 * len(imagens)
    entradas, corpo = [], b""
    for lado, dados in imagens:
        medida = 0 if lado >= 256 else lado  # 0 quer dizer 256
        # largura, altura, cores da paleta, reservado, planos, bits por pixel, tamanho e onde começa
        entradas.append(struct.pack("<BBBBHHII", medida, medida, 0, 0, 1, 32, len(dados), inicio + len(corpo)))
        corpo += dados
    return cabecalho + b"".join(entradas) + corpo


def conferir(caminho):
    """Lista as entradas de um .ico: lado, bytes e formato."""
    dados = caminho.read_bytes()
    reservado, tipo, quantas = struct.unpack("<HHH", dados[:6])
    if (reservado, tipo) != (0, 1):
        raise SystemExit(f"{caminho} não é um .ico.")
    print(f"{caminho.name}: {quantas} imagens, {len(dados)} bytes")
    for i in range(quantas):
        largura, _, _, _, _, bits, tamanho, inicio = struct.unpack("<BBBBHHII", dados[6 + 16 * i : 22 + 16 * i])
        formato = "PNG" if dados[inicio : inicio + 8] == ASSINATURA_PNG else "BMP"
        print(f"  {largura or 256:>3} px  {bits} bits  {tamanho:>6} bytes  {formato}")


def main():
    if "--conferir" in sys.argv[1:]:
        conferir(DESTINO)
        return
    imagens = []
    for lado, nome in IMAGENS:
        dados = (LOGO / nome).read_bytes()
        lido = lado_do_png(dados, nome)
        if lido != lado:
            raise SystemExit(f"{nome} tem {lido} px, e não {lado}.")
        imagens.append((lado, dados))
    DESTINO.write_bytes(montar(imagens))
    conferir(DESTINO)


if __name__ == "__main__":
    main()
