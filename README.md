# crytter

A compiled terminal emulator that runs in the browser. Drop-in replacement for xterm.js — no JavaScript runtime dependency, pure Rust compiled to WASM, rendered to Canvas2D.

The crab terminal. Because Rust.

**86KB WASM** | **37KB gzipped** | **xterm.js is 300KB gzipped**

## Quick Start

```bash
npm install crytter
```

```javascript
import init, { Terminal } from 'crytter';

await init();

const term = new Terminal();
term.open(document.getElementById('terminal'));

// Render loop (required — writes are batched via requestAnimationFrame)
function renderLoop() { term.render(); requestAnimationFrame(renderLoop); }
requestAnimationFrame(renderLoop);

// Write PTY output
const response = term.write(data);
if (response) ws.send(response); // device query responses go back to PTY

// Read keyboard input
term.onData((data) => ws.send(data));
```

## What Works

- **Shell**: bash, zsh, fish
- **TUI apps**: vim, htop, less, man, tmux
- **Modern tools**: claude-code, vttest
- **Colors**: 16 ANSI + 256 indexed + 24-bit RGB
- **Features**: scrollback (10K lines), cursor shapes, alt screen, scroll regions, bracket paste, app cursor mode
- **Keyboard**: full mapping including Ctrl, Alt, function keys, arrow keys (normal + application mode)
- **Device queries**: DA1, DA2, CPR, DSR, DECRQM, XTVERSION, window size report

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
brew install binaryen  # for wasm-opt

# Build
wasm-pack build crytter-wasm --target web --out-dir ../pkg --release
wasm-opt pkg/crytter_wasm_bg.wasm -Oz \
  --enable-bulk-memory --enable-nontrapping-float-to-int \
  -o pkg/crytter_wasm_bg.wasm

# Test
cargo test                          # 75 Rust tests
npx playwright test                 # 28+ browser tests (offline)
npx playwright test tui-stress      # TUI app tests (needs hermytt)
```

## API

### Terminal

```javascript
const term = new Terminal({ cols: 80, rows: 24 });

term.open(containerElement);     // Mount to DOM
term.write(data);                // Write PTY output (string)
term.writeBytes(uint8Array);     // Write PTY output (bytes)
term.render();                   // Draw if dirty (call from rAF)
term.fit();                      // Resize to container
term.resize(cols, rows);         // Resize to specific dimensions
term.reset();                    // Full terminal reset
term.refresh();                  // Force redraw

// Scrollback
term.scrollUp(lines);
term.scrollDown(lines);
term.scrollToBottom();

// Input
const data = term.handleKeyEvent(event);  // KeyboardEvent → escape sequence

// Callbacks
term.onTitleChange((title) => { ... });

// Properties
term.cols;          // number
term.rows;          // number
term.isScrolled;    // boolean
term.needsRender;   // boolean

// Debug
term.dumpGrid();    // Grid content as text
```

### Device Query Responses

`write()` returns a string when the PTY sends device queries (DA1, CPR, etc.). Send it back to the PTY:

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
- **Dirty tracking**: infrastructure for incremental updates

## Session Recording & Replay

Record a live terminal session, replay it offline. No network, no API calls, deterministic.

```bash
# Record
npx playwright test tests/e2e/record-session.spec.mjs

# Replay in tests
npx playwright test tests/e2e/replay-session.spec.mjs

# Visual replay in browser
open http://localhost:8080/www/replay.html
```

See [doc/testing-guide.md](doc/testing-guide.md) for details.

## The YTT Family

| Project | Role |
|---------|------|
| [hermytt](https://github.com/user/hermytt) | Transport-agnostic PTY multiplexer |
| **crytter** | WASM terminal emulator |
| [prytty](https://github.com/user/prytty) | Syntax highlighting for terminal output |

## License

MIT
