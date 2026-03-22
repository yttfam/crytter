# Contributing to crytter

## Getting Started

```bash
git clone <repo-url>
cd crytter

# Rust toolchain
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
brew install binaryen  # macOS — or your platform's equivalent

# Node (for Playwright tests)
npm install
npx playwright install chromium

# Build
wasm-pack build crytter-wasm --target web --out-dir ../pkg --release

# Test
cargo test && npx playwright test
```

## Project Structure

- `crates/crytter-vte/` — VTE parser wrapper. Thin layer over the `vte` crate.
- `crates/crytter-grid/` — Terminal state machine. This is where most escape sequence handling lives.
- `crates/crytter-render/` — Canvas2D renderer. Color palette, font handling, cursor drawing.
- `crates/crytter-input/` — Keyboard to escape sequence mapping.
- `crytter-wasm/` — WASM entry point. Glues the crates together and exposes the JS API.
- `www/` — Demo/test HTML pages.
- `tests/e2e/` — Playwright browser tests.
- `tests/fixtures/` — Recorded terminal sessions for offline testing.
- `doc/` — Guides and documentation.

## Development Workflow

1. Make changes to Rust code
2. `cargo test` — must pass
3. Build WASM: `wasm-pack build crytter-wasm --target web --out-dir ../pkg`
4. `npx playwright test` — must pass
5. Test visually: `python3 serve.py 8080` then open `http://localhost:8080`

## Adding Terminal Escape Sequences

Most work happens in `crates/crytter-grid/src/term.rs`:

- CSI sequences: add a match arm in the `csi()` method
- DEC private modes: add to `dec_set()` / `dec_reset()`
- OSC sequences: add to `osc()`
- ESC sequences: add to `esc()`

Always add a test in `crates/crytter-grid/tests/integration.rs`.

Reference: [xterm ctlseqs](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)

## Testing

Three layers:

- **Rust unit tests**: `cargo test` — fast, no browser
- **Playwright offline**: `npx playwright test tests/e2e/replay-session.spec.mjs` — replays recorded sessions, no network
- **Playwright live**: `npx playwright test tests/e2e/tui-stress.spec.mjs` — needs hermytt running

To record a new test fixture:
```bash
npx playwright test tests/e2e/record-session.spec.mjs
```

See [doc/testing-guide.md](doc/testing-guide.md) for the full guide.

## Code Style

- No over-engineering. Minimum code for the feature.
- No unnecessary abstractions. Three similar lines > one premature helper.
- Bounds-check everything that touches grid indices.
- Cap repeat counts (MAX_REPEAT) on any looping CSI parameter.
- Test adversarial inputs: zero-size terminals, huge params, binary garbage.

## Security

Every phase gets an adversarial + OWASP review. Key principles:

- All grid access is bounds-checked (returns default for OOB)
- CSI param counts capped to prevent DoS loops
- OSC title length capped (4096 bytes)
- Terminal dimensions clamped (1..10,000)
- No user-controlled data in device query responses (static strings + clamped integers)

If you add a new escape sequence handler, think about what happens with:
- Params of 0, 65535, or negative values
- Rapid repeated invocation
- Interaction with wrap_pending state

## Pull Requests

- Keep PRs focused. One feature or fix per PR.
- Include tests. Rust test for grid logic, Playwright test if it affects rendering.
- Run `cargo test && npx playwright test` before submitting.
- Don't include `pkg/`, `target/`, `node_modules/`, or `test-results/` in commits.
