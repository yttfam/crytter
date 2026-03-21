#!/usr/bin/env python3
"""Dev server with correct WASM MIME type."""
import http.server
import sys


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        '.wasm': 'application/wasm',
        '.js': 'application/javascript',
    }

    def do_GET(self):
        if self.path == '/':
            self.send_response(302)
            self.send_header('Location', '/www/')
            self.end_headers()
        else:
            super().do_GET()


port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
print(f"Serving on http://0.0.0.0:{port}")
http.server.HTTPServer(('0.0.0.0', port), Handler).serve_forever()
