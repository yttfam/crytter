// @ts-check
import { test, expect } from '@playwright/test';

const HERMYTT_HOST = 'localhost';
const HERMYTT_PORT = 7777;
const TOKEN = 'hermytt-test-token';

test.describe('debug claude rendering', () => {
  test.beforeEach(async ({ request }) => {
    try {
      const resp = await request.get(`http://${HERMYTT_HOST}:${HERMYTT_PORT}/info`, {
        headers: { 'X-Hermytt-Key': TOKEN },
      });
      if (resp.status() !== 200) test.skip();
    } catch { test.skip(); }
  });

  test('capture claude output bytes', async ({ page }) => {
    test.setTimeout(60_000);

    // Load test harness with raw byte logging
    await page.goto('/www/test.html');
    await page.waitForFunction(() => window.__ready === true, null, { timeout: 10_000 });

    // Intercept writes to log raw data
    await page.evaluate(({ host, port, token }) => {
      window.__rawLog = [];

      // Create session + connect
      fetch(`http://${host}:${port}/session`, {
        method: 'POST',
        headers: { 'X-Hermytt-Key': token },
      }).then(r => r.json()).then(({ id }) => {
        const ws = new WebSocket(`ws://${host}:${port}/ws/${id}`);
        window.__ws = ws;

        ws.onopen = () => ws.send(token);
        ws.onmessage = (e) => {
          const data = e.data;
          if (data === 'auth:ok') {
            ws.send(JSON.stringify({ resize: [window.__term.cols, window.__term.rows] }));
            window.__authed = true;
            return;
          }
          // Log raw bytes for analysis
          window.__rawLog.push(data);
          const response = window.__term.write(data);
          if (response && ws.readyState === 1) ws.send(response);
        };
      });
    }, { host: HERMYTT_HOST, port: HERMYTT_PORT, token: TOKEN });

    // Wait for auth
    await page.waitForFunction(() => window.__authed, null, { timeout: 10_000 });
    await page.waitForTimeout(1000);

    // Launch interactive claude, accept trust prompt, send a message
    await page.evaluate(() => {
      const cmd = 'claude\r';
      for (const ch of cmd) window.__ws.send(ch);
    });
    await page.waitForTimeout(5000);

    // Press Enter to accept trust prompt
    await page.evaluate(() => window.__ws.send('\r'));
    await page.waitForTimeout(5000);

    // Send a short message
    await page.evaluate(() => {
      const msg = 'say hi\r';
      for (const ch of msg) window.__ws.send(ch);
    });
    await page.waitForTimeout(15000);

    // Dump raw log looking for "66" or unhandled sequences
    const log = await page.evaluate(() => {
      return window.__rawLog.map(s => {
        // Convert to hex-annotated form for escape sequences
        return Array.from(s).map(c => {
          const code = c.charCodeAt(0);
          if (code < 32 || code === 127) return `\\x${code.toString(16).padStart(2, '0')}`;
          if (code === 0x1b) return '\\e';
          return c;
        }).join('');
      });
    });

    // Search for suspicious sequences
    const suspicious = log.filter(line =>
      line.includes('66') || line.includes('ransmut') || line.includes('erebrat') ||
      line.includes('\\x1b]') || line.includes('\\x1b[1A') || line.includes('\\x1b[2K')
    );
    console.log('=== Suspicious entries (66, spinner, OSC, cursor-up, erase-line) ===');
    for (const s of suspicious.slice(0, 30)) {
      console.log(s.substring(0, 300));
    }

    console.log(`\n=== Total log entries: ${log.length} ===`);
    console.log('\n=== Last 10 log entries ===');
    for (let i = Math.max(0, log.length - 10); i < log.length; i++) {
      console.log(`[${i}] ${log[i].substring(0, 300)}`);
    }

    await page.screenshot({ path: 'test-results/claude-debug.png' });
  });
});
