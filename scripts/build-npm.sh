#!/bin/bash
set -euo pipefail

# Build optimized WASM package for npm
echo "Building WASM..."
wasm-pack build crytter-wasm --target web --out-dir ../pkg --release

echo "Optimizing with wasm-opt..."
wasm-opt pkg/crytter_wasm_bg.wasm -Oz \
  --enable-bulk-memory --enable-nontrapping-float-to-int \
  -o pkg/crytter_wasm_bg.wasm

# Copy README into package
cp README.md pkg/README.md

# Restore our package.json (wasm-pack overwrites it)
cat > pkg/package.json << 'PKGJSON'
{
  "name": "crytter",
  "version": "0.1.0",
  "description": "WASM terminal emulator. 130KB drop-in xterm.js replacement. Rust + Canvas2D.",
  "type": "module",
  "main": "crytter_wasm.js",
  "types": "crytter_wasm.d.ts",
  "files": [
    "crytter_wasm_bg.wasm",
    "crytter_wasm.js",
    "crytter_wasm.d.ts",
    "crytter_wasm_bg.wasm.d.ts",
    "README.md"
  ],
  "keywords": [
    "terminal", "emulator", "wasm", "webassembly",
    "xterm", "canvas", "rust", "tty", "pty", "vte"
  ],
  "repository": {
    "type": "git",
    "url": "https://github.com/calibrae/crytter"
  },
  "homepage": "https://github.com/calibrae/crytter",
  "license": "MIT",
  "author": "Nicolas Bousquet <dev@calii.net>",
  "sideEffects": false
}
PKGJSON

echo ""
echo "Package ready in pkg/"
ls -lh pkg/crytter_wasm_bg.wasm
echo ""
echo "To publish: cd pkg && npm publish"
echo "To dry-run: cd pkg && npm pack --dry-run"
