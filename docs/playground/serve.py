#!/usr/bin/env python3
"""ローカルで playground を試す静的サーバー。Z3（z3-solver の WASM）が SharedArrayBuffer を使うので COOP/COEP を付ける。
GitHub Pages では coi-serviceworker.min.js が同じことをする。
使い方: python3 docs/playground/serve.py [port]  →  http://127.0.0.1:8765/docs/playground/draft.html"""
import http.server, os, sys

class H(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

os.chdir(os.path.join(os.path.dirname(__file__), "..", ".."))
port = int(sys.argv[1]) if len(sys.argv) > 1 else 8765
http.server.ThreadingHTTPServer(("127.0.0.1", port), H).serve_forever()
