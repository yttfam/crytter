// @ts-check
import { test } from '@playwright/test';
import { writeFileSync, mkdirSync } from 'fs';

const HERMYTT_HOST = 'localhost';
const HERMYTT_PORT = 7777;
const TOKEN = 'hermytt-test-token';

/**
 * Records a real claude session as a replayable fixture.
 * Run once to generate the fixture, then use it for offline tests.
 *
 * Usage: npx playwright test tests/e2e/record-session.spec.mjs
 */
test('record claude session as fixture', async ({ page, request }) => {
  test.setTimeout(120_000);

  try {
    const resp = await request.get(`http://${HERMYTT_HOST}:${HERMYTT_PORT}/info`, {
      headers: { 'X-Hermytt-Key': TOKEN },
    });
    if (resp.status() !== 200) test.skip();
  } catch { test.skip(); }

  await page.goto('/www/test.html');
  await page.waitForFunction(() => window.__ready === true, null, { timeout: 10_000 });

  // Connect and record all WS messages with timestamps
  await page.evaluate(({ host, port, token }) => {
    window.__recording = [];
    window.__startTime = Date.now();

    return new Promise((resolve, reject) => {
      const ws = new WebSocket(`ws://${host}:${port}/ws`);
      window.__ws = ws;
      ws.onopen = () => ws.send(token);
      ws.onmessage = (e) => {
        const t = Date.now() - window.__startTime;
        if (e.data === 'auth:ok') {
          ws.send(JSON.stringify({ resize: [window.__term.cols, window.__term.rows] }));
          window.__recording.push({ t, type: 'auth' });
          resolve();
          return;
        }
        // Record the raw PTY output
        window.__recording.push({ t, type: 'output', data: e.data });
        const response = window.__term.write(e.data);
        if (response && ws.readyState === 1) {
          window.__recording.push({ t, type: 'response', data: response });
          ws.send(response);
        }
      };
      ws.onerror = () => reject(new Error('WS error'));
      setTimeout(() => reject(new Error('timeout')), 10000);
    });
  }, { host: HERMYTT_HOST, port: HERMYTT_PORT, token: TOKEN });

  await page.waitForTimeout(1000);

  // Record user inputs too
  const sendInput = async (data, label) => {
    const t = await page.evaluate(({ data }) => {
      const t = Date.now() - window.__startTime;
      window.__recording.push({ t, type: 'input', data });
      window.__ws.send(data);
      return t;
    }, { data });
    console.log(`[${t}ms] input: ${label}`);
  };

  // Launch claude
  for (const c of 'claude') await sendInput(c, c);
  await sendInput('\r', 'Enter');
  await page.waitForTimeout(6000);

  // Trust folder
  await sendInput('1', '1');
  await page.waitForTimeout(500);
  await sendInput('\r', 'Enter');
  await page.waitForTimeout(8000);

  // First message
  for (const c of 'Hello') await sendInput(c, c);
  await sendInput('\r', 'Enter');
  await page.waitForTimeout(20000);

  // Second message
  for (const c of 'How are you?') await sendInput(c, c);
  await sendInput('\r', 'Enter');
  await page.waitForTimeout(20000);

  // Ctrl+C to exit
  await sendInput('\x03', 'Ctrl+C');
  await page.waitForTimeout(3000);

  // Save the recording
  const recording = await page.evaluate(() => {
    return {
      cols: window.__term.cols,
      rows: window.__term.rows,
      events: window.__recording,
    };
  });

  mkdirSync('tests/fixtures', { recursive: true });
  writeFileSync(
    'tests/fixtures/claude-session.json',
    JSON.stringify(recording, null, 2),
  );

  console.log(`\nRecorded ${recording.events.length} events`);
  console.log(`Saved to tests/fixtures/claude-session.json`);
});
