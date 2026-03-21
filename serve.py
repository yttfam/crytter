#!/usr/bin/env python3
"""Dev server with correct WASM MIME type."""
import http.server
import sys

handler = http.server.SimpleHTTPRequestHandler
handler.extensions_map['.wasm'] = 'application/wasm'
handler.extensions_map['.js'] = 'application/javascript'

port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
print(f"Serving on http://0.0.0.0:{port}")
http.server.HTTPServer(('0.0.0.0', port), handler).serve_forever()
