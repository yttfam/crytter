# Crytter

A compiled terminal emulator that runs in the browser. Drop-in replacement for xterm.js — no JavaScript runtime dependency, pure compiled code rendered to Canvas.

The crab terminal. Because Rust.

## The Problem

Every browser-based terminal today depends on xterm.js — 50K lines of JavaScript, loaded from a CDN or bundled. It works, but it's JS parsing escape sequences character by character. We can do better with compiled code.

## The Idea

A Rust terminal emulator compiled to WASM, rendering to Canvas2D. Ships as a single `.wasm` + thin JS glue (~50 lines). Exposes the same API surface as xterm.js so existing projects can swap it in.

```
PTY bytes → WASM (VTE parser + grid state) → Canvas2D render → pixels
keyboard → WASM (key mapping) → escape sequences → PTY
```

## Architecture

```
crytter/
├── crates/
│   ├── crytter-vte/        # Fork/wrap of alacritty_terminal's VTE parser
│   ├── crytter-grid/       # Cell grid, attributes, scrollback, terminal modes
│   ├── crytter-render/     # Canvas2D renderer via web-sys
│   └── crytter-input/      # Keyboard/mouse → escape sequence mapping
├── crytter-wasm/            # WASM entry point, JS glue, public API
├── www/                     # Demo page
└── pkg/                     # Built WASM package (npm-publishable)
```

## Key Design Decisions

### Use Alacritty's VTE parser, not from scratch
- `vte` crate: zero-alloc state machine, ~1500 lines, battle-tested
- Parses CSI, OSC, DCS, ESC sequences
- Does NOT manage grid state — that's our job
- Saves 2-3 months of work

### Canvas2D, not WebGL/WebGPU
- Terminal is monospace text in a grid. Not a 3D scene.
- `fillText` on a 2D canvas at 60fps can render 80x24 in microseconds
- No GPU glyph atlas, no shader pipeline, no texture management
- Uses the browser's native font renderer (subpixel, ligatures, fallbacks — free)
- If Canvas2D becomes a bottleneck (it won't for terminals), upgrade to WebGL later

### Minimal JS glue
- WASM can't touch the DOM directly (thanks, standards committee)
- Thin JS layer: create canvas, forward keyboard events, expose API
- Target: <100 lines of JS glue
- Everything else runs in WASM

### xterm.js-compatible API
- `new Terminal(options)` → create instance
- `.open(container)` → mount to DOM
- `.write(data)` → feed PTY output
- `.onData(callback)` → receive keyboard input
- `.fit()` → resize to container
- Addons: fit, web-links (built-in, not separate packages)

## What Needs Building

### Phase 1: Parser + Grid (Month 1-2)
- Integrate `vte` crate (or `alacritty_terminal` grid directly)
- Cell grid with attributes (fg, bg, bold, italic, underline, etc.)
- 16 + 256 + truecolor support
- Cursor state, movement, save/restore
- Scroll regions, line wrap, insert/delete
- Alternate screen buffer
- Scrollback ring buffer
- Terminal modes (origin, wrap, insert, bracket paste)
- Tab stops

### Phase 2: Renderer (Month 2-3)
- Canvas2D rendering: draw cell grid to canvas
- Font measurement (char width/height from browser)
- Dirty tracking: only redraw changed cells
- Cursor rendering (block, underline, bar) with blink
- Selection highlighting
- Scroll position management
- Resize handling (reflow text)

### Phase 3: Input (Month 3-4)
- Keyboard event → escape sequence mapping
- Modifier keys (ctrl, alt, shift combinations)
- Function keys, arrows, home/end/pgup/pgdn
- Keypad (application mode vs normal)
- Paste handling (bracket paste mode)
- Mouse reporting (X10, SGR, button tracking)
- IME support (composing input for CJK) — hard, do last

### Phase 4: Integration + Polish (Month 4-5)
- WebSocket transport (built-in, not addon)
- npm package, `import { Terminal } from 'crytter'`
- xterm.js API compatibility layer
- URL detection (clickable links)
- Search in scrollback
- Copy/paste with selection
- Performance: benchmark against xterm.js with `cat /dev/urandom`

### Phase 5: Mobile (Month 5-6)
- Touch events → keyboard input
- Virtual keyboard integration
- Pinch-to-zoom font size
- iOS Safari viewport hell (we know this one)

## Prior Art

| Project | Status | Notes |
|---------|--------|-------|
| xterm.js | Production | JS, 50K lines, the standard |
| Alacritty | Production | Rust native, GPU renderer, VTE parser is gold |
| Rio | Active desktop, no web | Rust + WebGPU, Sugarloaf renderer, WASM planned but not shipped |
| @wasmer/wasm-terminal | Abandoned | Wraps xterm.js anyway |
| workflow-terminal | Niche | Unifies termion + xterm.js, not a replacement |

## Key Risks

1. **Unicode/grapheme clusters**: CJK wide chars, combining characters, emoji with ZWJ — each is a rabbit hole
2. **IME composition**: required for CJK input, hard to get right in WASM
3. **Browser font rendering**: Canvas2D `fillText` works but measuring char widths precisely across platforms is tricky
4. **The last 5%**: 95% of terminal apps work after 4 months. vim/tmux/htop edge cases take another 6.
5. **Performance parity**: xterm.js is heavily optimized. Beating it with WASM is possible but not guaranteed.

## Tech Stack

- Rust + wasm-bindgen + web-sys
- `vte` or `alacritty_terminal` crate for parsing
- Canvas2D for rendering
- wasm-pack for building
- npm for distribution

## Success Criteria

```bash
npm install crytter
```

```javascript
import { Terminal } from 'crytter';
const term = new Terminal();
term.open(document.getElementById('terminal'));
ws.onmessage = (e) => term.write(e.data);
term.onData((data) => ws.send(data));
```

Works with: bash, zsh, vim, tmux, htop, less, man, ssh, claude-code.
Size: <500KB WASM + <100 lines JS glue.
Performance: equal or better than xterm.js.

## Cali's Preferences

- Rust workspace
- No unnecessary GPU complexity — Canvas2D first
- npm-publishable package
- Must work on Safari (iOS included)
- First consumer: hermytt
