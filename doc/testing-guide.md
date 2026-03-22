# Crytter Testing Guide

How to set up Playwright for testing crytter (standalone or integrated with hermytt), and how the offline event capture/replay system works.

## Prerequisites

```bash
npm install --save-dev @playwright/test
npx playwright install chromium
```

## Test Architecture

Crytter has three layers of tests:

### 1. Rust unit/integration tests (`cargo test`)
Pure terminal logic — no browser needed.
- VTE parser (crytter-vte)
- Grid state machine (crytter-grid)
- Input mapping (crytter-input)
- Color palette (crytter-render)

### 2. Playwright e2e tests (headless Chromium)
WASM running in a real browser. Two modes:
- **Offline**: replay recorded sessions from fixture files (no network)
- **Online**: connect to hermytt for live PTY tests

### 3. Visual replay (`/www/replay.html`)
Browser-based replay of recorded sessions with speed control. For manual visual inspection.

---

## Setting Up Playwright for hermytt + crytter

### Project structure

```
your-project/
├── playwright.config.mjs
├── tests/
│   ├── e2e/
│   │   ├── terminal.spec.mjs    # offline WASM tests
│   │   ├── tui-stress.spec.mjs  # live TUI app tests (needs hermytt)
│   │   └── replay.spec.mjs      # offline fixture replay
│   └── fixtures/
│       └── claude-session.json   # recorded session
├── www/
│   ├── index.html               # main terminal page
│   └── test.html                # bare test harness
└── pkg/
    ├── crytter_wasm.js           # wasm-bindgen glue
    └── crytter_wasm_bg.wasm      # compiled WASM
```

### playwright.config.mjs

```javascript
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  timeout: 30_000,
  use: {
    baseURL: 'http://localhost:8081',
    headless: true,
  },
  // Static file server for WASM + HTML
  webServer: {
    command: 'python3 serve.py 8081',
    port: 8081,
    reuseExistingServer: true,
  },
  projects: [
    { name: 'chromium', use: { browserName: 'chromium' } },
  ],
});
```

### serve.py (dev server with WASM MIME type)

```python
#!/usr/bin/env python3
import http.server, sys

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
http.server.HTTPServer(('0.0.0.0', port), Handler).serve_forever()
```

### test.html (bare test harness)

Minimal page that loads crytter and exposes the terminal instance to Playwright:

```html
<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>test</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { background: #1e1e1e; }
  #terminal-container { width: 800px; height: 400px; }
</style>
</head>
<body>
<div id="terminal-container"></div>
<script type="module">
  import init, { Terminal } from '/pkg/crytter_wasm.js';
  async function setup() {
    await init();
    const term = new Terminal();
    term.open(document.getElementById('terminal-container'));
    function renderLoop() { term.render(); requestAnimationFrame(renderLoop); }
    requestAnimationFrame(renderLoop);
    window.__term = term;
    window.__ready = true;
  }
  setup().catch(e => { window.__error = e.message; });
</script>
</body>
</html>
```

---

## Writing Tests

### Offline test (no hermytt needed)

```javascript
import { test, expect } from '@playwright/test';

test('write and check grid', async ({ page }) => {
  await page.goto('/www/test.html');
  await page.waitForFunction(() => window.__ready === true, null, { timeout: 10_000 });

  // Write directly to terminal
  await page.evaluate(() => window.__term.write('Hello World'));
  await page.waitForTimeout(100); // wait for rAF

  // Check grid content
  const grid = await page.evaluate(() => window.__term.dumpGrid());
  expect(grid).toContain('Hello World');
});
```

### Live test (with hermytt)

```javascript
import { test, expect } from '@playwright/test';

const HERMYTT = { host: 'localhost', port: 7777, token: 'your-token' };

test.describe('live terminal', () => {
  test.beforeEach(async ({ request }) => {
    // Skip if hermytt isn't running
    try {
      const resp = await request.get(
        `http://${HERMYTT.host}:${HERMYTT.port}/info`,
        { headers: { 'X-Hermytt-Key': HERMYTT.token } }
      );
      if (resp.status() !== 200) test.skip();
    } catch { test.skip(); }
  });

  test('run a command', async ({ page }) => {
    await page.goto(
      `/www/index.html?host=${HERMYTT.host}&port=${HERMYTT.port}&token=${HERMYTT.token}`
    );
    await page.waitForFunction(
      () => document.getElementById('term-info')?.textContent?.includes('connected'),
      null, { timeout: 15_000 }
    );
    await page.waitForTimeout(500);

    await page.keyboard.type('echo hello');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(1000);

    // Canvas should have content
    const hasContent = await page.evaluate(() => {
      const canvas = document.querySelector('#terminal-container canvas');
      const ctx = canvas.getContext('2d');
      const data = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
      for (let i = 0; i < data.length; i += 4) {
        if (data[i] > 50 || data[i+1] > 50 || data[i+2] > 50) return true;
      }
      return false;
    });
    expect(hasContent).toBe(true);
  });
});
```

### Debugging hermytt integration

Common checklist when crytter + hermytt doesn't work:

```javascript
test('debug integration', async ({ page }) => {
  // Collect console errors
  const errors = [];
  page.on('console', msg => { if (msg.type() === 'error') errors.push(msg.text()); });
  page.on('pageerror', err => errors.push(err.message));

  await page.goto('/your-terminal-page.html');
  await page.waitForTimeout(5000);

  // 1. Did WASM load?
  const wasmLoaded = await page.evaluate(() => typeof window.__term !== 'undefined');
  console.log('WASM loaded:', wasmLoaded);

  // 2. Did canvas get created?
  const hasCanvas = await page.evaluate(
    () => document.querySelector('canvas') !== null
  );
  console.log('Canvas created:', hasCanvas);

  // 3. Canvas dimensions?
  const canvasSize = await page.evaluate(() => {
    const c = document.querySelector('canvas');
    return c ? { w: c.width, h: c.height } : null;
  });
  console.log('Canvas size:', canvasSize);

  // 4. Any pixels rendered?
  const hasPixels = await page.evaluate(() => {
    const c = document.querySelector('canvas');
    if (!c) return false;
    const ctx = c.getContext('2d');
    const data = ctx.getImageData(0, 0, c.width, c.height).data;
    for (let i = 0; i < data.length; i += 4) {
      if (data[i] > 50 || data[i+1] > 50 || data[i+2] > 50) return true;
    }
    return false;
  });
  console.log('Has rendered pixels:', hasPixels);

  // 5. Console errors?
  console.log('Errors:', errors);

  await page.screenshot({ path: 'debug-output.png' });
});
```

---

## Event Capture & Offline Replay System

### How it works

1. **Record**: run a real terminal session, capture every WebSocket message with timestamps
2. **Save**: write the recording as a JSON fixture
3. **Replay**: feed the recorded bytes into crytter's WASM terminal without any network

### Recording format

```json
{
  "cols": 151,
  "rows": 40,
  "events": [
    { "t": 0,    "type": "auth" },
    { "t": 150,  "type": "output", "data": "\u001b[1m\u001b[7m%..." },
    { "t": 1012, "type": "input",  "data": "c" },
    { "t": 1015, "type": "input",  "data": "l" },
    { "t": 5200, "type": "output", "data": "\u001b[?2026h..." },
    { "t": 5200, "type": "response", "data": "\u001b[?62;22c" }
  ]
}
```

- `t` — milliseconds since session start
- `type`:
  - `auth` — WebSocket authenticated
  - `output` — PTY output (bytes from server → terminal)
  - `input` — user input (bytes from terminal → server)
  - `response` — device query response (terminal → server, e.g. DA1, CPR)
- `data` — the raw string (escape sequences preserved as unicode escapes in JSON)

### Recording a session

```javascript
// In Playwright — connect to hermytt, capture everything
await page.evaluate(({ host, port, token }) => {
  window.__recording = [];
  window.__startTime = Date.now();

  const ws = new WebSocket(`ws://${host}:${port}/ws`);
  window.__ws = ws;
  ws.onopen = () => ws.send(token);
  ws.onmessage = (e) => {
    const t = Date.now() - window.__startTime;
    if (e.data === 'auth:ok') {
      ws.send(JSON.stringify({ resize: [window.__term.cols, window.__term.rows] }));
      window.__recording.push({ t, type: 'auth' });
      return;
    }
    window.__recording.push({ t, type: 'output', data: e.data });
    const response = window.__term.write(e.data);
    if (response) {
      window.__recording.push({ t, type: 'response', data: response });
      ws.send(response);
    }
  };
}, config);

// ... interact with terminal ...

// Save fixture
const recording = await page.evaluate(() => ({
  cols: window.__term.cols,
  rows: window.__term.rows,
  events: window.__recording,
}));
writeFileSync('tests/fixtures/my-session.json', JSON.stringify(recording, null, 2));
```

### Replaying a fixture

```javascript
import { readFileSync } from 'fs';

test('replay session', async ({ page }) => {
  const recording = JSON.parse(readFileSync('tests/fixtures/my-session.json', 'utf-8'));

  await page.goto('/www/test.html');
  await page.waitForFunction(() => window.__ready, null, { timeout: 10_000 });

  // Match recording dimensions
  await page.evaluate(({ cols, rows }) => window.__term.resize(cols, rows), recording);

  // Replay all output events
  const outputs = recording.events.filter(e => e.type === 'output');

  // Option A: instant replay (fast, for CI)
  await page.evaluate((outputs) => {
    for (const event of outputs) {
      window.__term.write(event.data);
    }
    window.__term.render();
  }, outputs);

  // Option B: timed replay (visual, for debugging)
  await page.evaluate(async ({ outputs, speed }) => {
    let lastT = outputs[0]?.t || 0;
    for (const event of outputs) {
      const delay = (event.t - lastT) / speed;
      if (delay > 1) await new Promise(r => setTimeout(r, delay));
      window.__term.write(event.data);
      lastT = event.t;
    }
  }, { outputs, speed: 10 }); // 10x speed

  // Verify
  const grid = await page.evaluate(() => window.__term.dumpGrid());
  expect(grid).toContain('expected text');
});
```

### Visual replay in browser

Open `/www/replay.html` — loads the fixture, provides play/pause/speed controls. No Playwright needed, just a browser.

---

## Crytter API Reference (for test authors)

### Terminal methods (available on `window.__term` in test.html)

| Method | Returns | Description |
|--------|---------|-------------|
| `write(data)` | `string \| undefined` | Write PTY output. Returns device query responses if any |
| `render()` | `boolean` | Render if dirty. Call from rAF loop. Returns true if drawn |
| `handleKeyEvent(event)` | `string \| undefined` | Convert KeyboardEvent to escape sequence |
| `fit()` | — | Resize terminal to fit container |
| `resize(cols, rows)` | — | Resize to specific dimensions |
| `reset()` | — | Full terminal reset |
| `refresh()` | — | Mark dirty for re-render |
| `dumpGrid()` | `string` | Dump grid content as text (for assertions) |
| `scrollUp(lines)` | — | Scroll into scrollback |
| `scrollDown(lines)` | — | Scroll toward live view |
| `scrollToBottom()` | — | Snap to live view |

### Properties

| Property | Type | Description |
|----------|------|-------------|
| `cols` | `number` | Current column count |
| `rows` | `number` | Current row count |
| `needsRender` | `boolean` | Whether a render is pending |
| `isScrolled` | `boolean` | Whether scrolled into scrollback |

---

## Running Tests

```bash
# Rust tests (always works, no browser)
cargo test

# Playwright offline tests (needs built WASM, no hermytt)
npx playwright test tests/e2e/terminal.spec.mjs
npx playwright test tests/e2e/replay-session.spec.mjs

# Playwright live tests (needs hermytt running)
npx playwright test tests/e2e/tui-stress.spec.mjs

# Everything
cargo test && npx playwright test

# Record a new session fixture
npx playwright test tests/e2e/record-session.spec.mjs

# Visual replay in browser
open http://localhost:8080/www/replay.html
```
