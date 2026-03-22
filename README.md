# crytter

A compiled terminal emulator that runs in the browser. Drop-in replacement for xterm.js — no JavaScript runtime dependency, pure Rust compiled to WASM, rendered to Canvas2D.

The crab terminal. Because Rust.

**130KB WASM** (full) | **99KB minimal** | **xterm.js is 300KB gzipped**

## Quick Start

```bash
npm install crytter
```

```javascript
import init, { Terminal } from 'crytter';

await init();

const term = new Terminal({
  fontFamily: "Menlo, Monaco, 'Courier New', monospace",
  fontSize: 14,
});
term.open(document.getElementById('terminal'));

// Render loop (required — writes are batched via requestAnimationFrame)
function renderLoop() { term.render(); requestAnimationFrame(renderLoop); }
requestAnimationFrame(renderLoop);

// Cursor blink
setInterval(() => term.blinkCursor(), 530);

// Write PTY output — returns device query responses to send back
const response = term.write(data);
if (response) ws.send(response);

// Keyboard input
document.addEventListener('keydown', (e) => {
  const data = term.handleKeyEvent(e);
  if (data != null) {
    e.preventDefault();
    ws.send(data);
  }
});
```

## What Works

- **Shell**: bash, zsh, fish
- **TUI apps**: vim, htop, less, man, tmux
- **Modern tools**: claude-code, vttest
- **Colors**: 16 ANSI + 256 indexed + 24-bit RGB
- **Unicode**: CJK wide characters, emoji (via `unicode-width`)
- **Features**: scrollback (10K lines), cursor shapes + blink, alt screen, scroll regions, bracket paste, app cursor mode
- **Selection**: mouse select + auto-copy to clipboard, Cmd/Ctrl+C to copy
- **Links**: URL detection, Cmd/Ctrl+click to open
- **Search**: Cmd/Ctrl+F to search scrollback
- **Keyboard**: full mapping including Ctrl, Alt, function keys, arrow keys (normal + application mode)
- **Device queries**: DA1, DA2, CPR, DSR, DECRQM, XTVERSION, window size report
- **~70 xterm escape sequences** implemented (CSI, ESC, OSC, DEC private modes)

## Feature Flags

Compile-time opt-in to keep the binary lean:

| Feature | Default | Size impact | Description |
|---------|---------|-------------|-------------|
| `unicode-width` | on | +10KB | CJK/emoji proper width handling |
| `links` | on | +15KB | URL detection in terminal output |
| `search` | on | +16KB | Search across grid + scrollback |

```bash
# Full build (130KB) — all features
wasm-pack build crytter-wasm --release

# Minimal build (99KB) — core terminal only
wasm-pack build crytter-wasm --release -- --no-default-features

# Cherry-pick features
wasm-pack build crytter-wasm --release -- --no-default-features --features links
```

## Architecture

```
crytter/
├── crates/
│   ├── crytter-vte/        # VTE parser (wraps `vte` crate)
│   ├── crytter-grid/       # Cell grid, cursor, attributes, scrollback, terminal modes
│   ├── crytter-render/     # Canvas2D renderer, HiDPI, color palette, dirty tracking
│   └── crytter-input/      # Keyboard/mouse → escape sequence mapping
├── crytter-wasm/            # WASM entry point, xterm.js-compatible API
├── www/                     # Demo pages
│   ├── index.html           # Live terminal (connects to hermytt)
│   ├── replay.html          # Offline session replay with speed control
│   └── test.html            # Bare test harness for Playwright
├── tests/
│   ├── e2e/                 # Playwright browser tests
│   └── fixtures/            # Recorded terminal sessions for offline replay
└── doc/                     # Guides
```

## Building

```bash
# Prerequisites
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
brew install binaryen  # for wasm-opt (macOS)

# Build
wasm-pack build crytter-wasm --target web --out-dir ../pkg --release
wasm-opt pkg/crytter_wasm_bg.wasm -Oz \
  --enable-bulk-memory --enable-nontrapping-float-to-int \
  -o pkg/crytter_wasm_bg.wasm

# Test
cargo test                          # 94 Rust tests
npx playwright test                 # 28 browser tests (offline)
npx playwright test tui-stress      # TUI app tests (needs hermytt)
```

## API

### Constructor Options

```javascript
const term = new Terminal({
  cols: 80,                          // default 80
  rows: 24,                         // default 24
  fontFamily: "Menlo, monospace",    // default Menlo
  fontSize: 14,                     // default 14
});
```

### Terminal Methods

```javascript
term.open(containerElement);     // Mount to DOM (creates canvas)
term.write(data);                // Write PTY output (string), returns responses
term.writeBytes(uint8Array);     // Write PTY output (bytes), returns responses
term.render();                   // Draw if dirty (call from rAF)
term.fit();                      // Resize to container
term.resize(cols, rows);         // Resize to specific dimensions
term.reset();                    // Full terminal reset
term.refresh();                  // Force redraw
term.blinkCursor();              // Toggle cursor blink (call from setInterval)

// Scrollback
term.scrollUp(lines);
term.scrollDown(lines);
term.scrollToBottom();

// Selection
term.mouseDown(x, y);           // Start selection (pixel coords)
term.mouseMove(x, y);           // Extend selection
term.mouseUp();                  // Finalize selection
term.getSelection();             // Get selected text
term.copySelection();            // Get selected text (alias)
term.clearSelection();           // Clear selection

// Links (requires "links" feature)
term.getUrlAt(x, y);            // URL at pixel position, or null

// Search (requires "search" feature)
term.search(needle);             // Returns JSON array of matches

// Input
term.handleKeyEvent(event);      // KeyboardEvent → escape sequence string

// Callbacks
term.onTitleChange(callback);    // Terminal title changed

// Properties
term.cols;                       // number
term.rows;                       // number
term.isScrolled;                 // boolean
term.needsRender;                // boolean
term.hasSelection;               // boolean

// Debug
term.dumpGrid();                 // Grid content as text
```

### Device Query Responses

`write()` returns a string when the PTY sends device queries (DA1, CPR, etc.). **You must send it back to the PTY** or apps will hang:

```javascript
ws.onmessage = (e) => {
  const response = term.write(e.data);
  if (response) ws.send(response);
};
```

## Performance

- **rAF batching**: multiple `write()` calls per frame coalesce into one render
- **Color cache**: pre-computed CSS strings for 256 indexed colors (zero allocation)
- **State minimization**: tracks current Canvas2D font/fill to skip redundant calls
- **Wide char rendering**: spacer cells skipped, wide chars drawn at double width
- **Scroll stability**: user scroll position preserved during streaming output

## Session Recording & Replay

Record a live terminal session as a JSON fixture, replay it offline. No network, no API calls, deterministic.

```bash
# Record a session (needs hermytt)
npx playwright test tests/e2e/record-session.spec.mjs

# Replay in tests (offline)
npx playwright test tests/e2e/replay-session.spec.mjs

# Visual replay in browser
open http://localhost:8080/www/replay.html
```

See [doc/testing-guide.md](doc/testing-guide.md) for the full testing guide.

## The YTT Family

| Project | | Role |
|---------|---|------|
| **hermytt** | 🏔️ | Transport-agnostic PTY multiplexer |
| **crytter** | 🖥️ | WASM terminal emulator |
| **prytty** | ✨ | Syntax highlighting for terminal output |

## License

MIT
