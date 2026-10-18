"""Servidor local do rasterizador: serve a pasta design/ (só leitura) e grava os PNGs que a página
rasterizar.html manda por POST em /salvar/<caminho relativo à pasta de saída>.

Uso (da raiz do projeto):  python design\\ferramentas\\rasterizar\\servidor.py <pasta de saída> [porta]
Depois abra http://127.0.0.1:<porta>/ferramentas/rasterizar/rasterizar.html?lista=<lista.json> num navegador
(o embutido do Claude ou o Edge) e espere o título da página virar "pronto". Só biblioteca padrão; escuta só em
127.0.0.1 e só grava .png dentro da pasta de saída."""
import http.server
import sys
from pathlib import Path

DESIGN = Path(__file__).resolve().parents[2]
SAIDA = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else DESIGN / "saida"
PORTA = int(sys.argv[2]) if len(sys.argv) > 2 else 8766


class Manipulador(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(DESIGN), **kwargs)

    def do_POST(self):  # noqa: N802 (nome do http.server)
        if not self.path.startswith("/salvar/"):
            self.send_error(404)
            return
        relativo = self.path[len("/salvar/"):].split("?", 1)[0]
        destino = (SAIDA / relativo).resolve()
        if SAIDA not in destino.parents or destino.suffix.lower() != ".png":
            self.send_error(403, "fora da pasta de saída ou não é .png")
            return
        tamanho = int(self.headers.get("Content-Length", "0"))
        dados = self.rfile.read(tamanho)
        if not dados.startswith(b"\x89PNG\r\n\x1a\n"):
            self.send_error(400, "não é PNG")
            return
        destino.parent.mkdir(parents=True, exist_ok=True)
        destino.write_bytes(dados)
        self.send_response(204)
        self.end_headers()

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, formato, *args):
        pass


if __name__ == "__main__":
    SAIDA.mkdir(parents=True, exist_ok=True)
    print(f"servindo {DESIGN} em http://127.0.0.1:{PORTA}/ e gravando em {SAIDA}", flush=True)
    http.server.ThreadingHTTPServer(("127.0.0.1", PORTA), Manipulador).serve_forever()
