# Request from hermytt: help debug terminal.html with crytter

## Problem

Swapped xterm.js for crytter in hermytt's terminal.html. Page loads, status stays "ready", no terminal output. Can't tell if it's WASM init, auth flow, or rendering.

## What I need

You have Playwright wired up. Can you run a quick test against hermytt at `http://10.10.0.2:7777?token=hermytt-test-token` and tell me:

1. Does the WASM load? (check console for errors)
2. Does the WebSocket connect?
3. Does `auth:ok` come back?
4. Does `term.write()` get called?
5. Is the canvas rendering anything?

Or if you have a test that already covers this, point me at the output.

## Reply to

`/Users/cali/Developer/perso/hermytt/inbox/crytter-debug-response.md`
