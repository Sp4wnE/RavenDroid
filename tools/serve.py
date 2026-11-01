#!/usr/bin/env python3
"""Desktop verification server mirroring the APK's decompressed local asset responses."""
import argparse
import gzip
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import io
import json
import mimetypes
from pathlib import Path
import shutil
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1] / 'app/src/main/assets'


class Parts(io.RawIOBase):
    def __init__(self, paths):
        super().__init__()
        self.paths = iter(paths)
        self.current = None

    def readable(self): return True

    def readinto(self, buffer):
        while True:
            if self.current is None:
                path = next(self.paths, None)
                if path is None: return 0
                self.current = path.open('rb')
            n = self.current.readinto(buffer)
            if n: return n
            self.current.close()
            self.current = None

    def close(self):
        if self.current: self.current.close()
        super().close()


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        path = unquote(urlsplit(self.path).path)
        if path == '/': path = '/web/index.html'
        target = (ROOT / path.lstrip('/')).resolve()
        if not target.is_relative_to(ROOT.resolve()):
            self.send_error(403); return
        manifest = json.loads((ROOT / 'game/manifest.json').read_text())
        try:
            if path == '/game/game.data':
                parts = Parts([ROOT / 'game' / name for name in manifest['dataParts']])
                stream = gzip.GzipFile(fileobj=io.BufferedReader(parts, 65536))
                size, mime = manifest['dataSize'], 'application/octet-stream'
            elif path == '/game/game.wasm':
                stream = gzip.open(ROOT / 'game/game.wasm.bin', 'rb')
                size, mime = manifest['wasmSize'], 'application/wasm'
            else:
                stream = target.open('rb')
                size, mime = target.stat().st_size, mimetypes.guess_type(target.name)[0] or 'application/octet-stream'
        except OSError:
            self.send_error(404); return
        self.send_response(200)
        self.send_header('Content-Type', mime)
        self.send_header('Content-Length', str(size))
        self.send_header('Cache-Control', 'no-store')
        self.end_headers()
        try:
            with stream: shutil.copyfileobj(stream, self.wfile, 65536)
        except (BrokenPipeError, ConnectionResetError): pass


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--port', type=int, default=8080)
    args = parser.parse_args()
    server = ThreadingHTTPServer(('127.0.0.1', args.port), Handler)
    print(f'Listening on http://127.0.0.1:{args.port}', flush=True)
    server.serve_forever()
