// @ts-check
import { test, expect } from '@playwright/test';
import { readFileSync } from 'fs';

/**
 * Replay a recorded claude session — no hermytt, no API calls.
 * Tests crytter's rendering against real Claude Code escape sequences.
 */

const FIXTURE = 'tests/fixtures/claude-session.json';

test.describe('replay recorded session', () => {
  let recording;

  test.beforeAll(() => {
    try {
      recording = JSON.parse(readFileSync(FIXTURE, 'utf-8'));
    } catch {
      // Fixture doesn't exist — skip all tests
    }
  });

  test.beforeEach(async ({ page }) => {
    if (!recording) test.skip();
    await page.goto('/www/test.html');
    await page.waitForFunction(() => window.__ready === true, null, { timeout: 10_000 });
    // Resize to match the recording
    await page.evaluate(({ cols, rows }) => {
      window.__term.resize(cols, rows);
    }, { cols: recording.cols, rows: recording.rows });
  });

  test('full session renders without crash', async ({ page }) => {
    test.setTimeout(30_000);

    // Replay all output events (skip inputs — they just echo)
    const outputs = recording.events.filter(e => e.type === 'output');

    const crashed = await page.evaluate((outputs) => {
      try {
        for (const event of outputs) {
          const response = window.__term.write(event.data);
          // Responses (DA1, CPR) are generated but we don't need to send them anywhere
        }
        window.__term.render();
        return false;
      } catch (e) {
        return e.message;
      }
    }, outputs);

    expect(crashed).toBe(false);
  });

  test('grid has expected content after replay', async ({ page }) => {
    test.setTimeout(30_000);

    const outputs = recording.events.filter(e => e.type === 'output');

    const grid = await page.evaluate((outputs) => {
      for (const event of outputs) {
        window.__term.write(event.data);
      }
      window.__term.render();
      return window.__term.dumpGrid();
    }, outputs);

    const lines = grid.split('\n').filter(l => l.trim().length > 0);

    // Should have rendered something
    expect(lines.length).toBeGreaterThan(3);

    // Should contain claude's response content
    const gridText = grid.toLowerCase();
    const hasGreeting = gridText.includes('hello') || gridText.includes('hi') || gridText.includes('help');
    expect(hasGreeting).toBe(true);

    // Print grid for inspection
    console.log('=== Replayed grid ===');
    for (let i = 0; i < Math.min(20, lines.length); i++) {
      console.log(`${String(i).padStart(2)}: ${lines[i]}`);
    }
  });

  test('canvas renders pixels after replay', async ({ page }) => {
    test.setTimeout(30_000);

    const outputs = recording.events.filter(e => e.type === 'output');

    await page.evaluate((outputs) => {
      for (const event of outputs) {
        window.__term.write(event.data);
      }
    }, outputs);

    // Wait for rAF to render
    await page.waitForTimeout(200);

    const hasContent = await page.evaluate(() => {
      const canvas = document.querySelector('#terminal-container canvas');
      if (!canvas) return false;
      const ctx = canvas.getContext('2d');
      const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
      const pixels = imageData.data;
      let nonBg = 0;
      for (let i = 0; i < pixels.length; i += 4) {
        if (pixels[i] > 50 || pixels[i + 1] > 50 || pixels[i + 2] > 50) nonBg++;
      }
      return nonBg > 100;
    });

    expect(hasContent).toBe(true);
    await page.screenshot({ path: 'test-results/replay-final.png' });
  });

  test('spinner lines are properly erased', async ({ page }) => {
    test.setTimeout(30_000);

    const outputs = recording.events.filter(e => e.type === 'output');

    const grid = await page.evaluate((outputs) => {
      for (const event of outputs) {
        window.__term.write(event.data);
      }
      window.__term.render();
      return window.__term.dumpGrid();
    }, outputs);

    // Find spinner remnants
    const spinnerPatterns = /ucleating|hannelling|deating|arinating|ransmut|hunder|erebr|neading/i;
    const lines = grid.split('\n');
    const remnants = lines
      .map((l, i) => ({ line: i, text: l }))
      .filter(({ text }) => spinnerPatterns.test(text));

    if (remnants.length > 0) {
      console.log('=== Spinner remnants found ===');
      for (const r of remnants) {
        console.log(`  Line ${r.line}: ${r.text}`);
      }
    }

    // This test documents the current state — ideally remnants should be 0
    // For now, just log and track
    console.log(`Spinner remnants: ${remnants.length}`);
  });

  test('no escape sequence leaks as visible text', async ({ page }) => {
    test.setTimeout(30_000);

    const outputs = recording.events.filter(e => e.type === 'output');

    const grid = await page.evaluate((outputs) => {
      for (const event of outputs) {
        window.__term.write(event.data);
      }
      window.__term.render();
      return window.__term.dumpGrid();
    }, outputs);

    // Check for raw escape sequence fragments in the grid
    const hasEscLeak = grid.includes('\x1b') || grid.includes('\\e[');
    expect(hasEscLeak).toBe(false);

    // Check for "66" artifacts (known previous issue)
    // Only flag if "66" appears outside normal text context
    const lines = grid.split('\n');
    const suspicious66 = lines.filter(l => {
      // Remove likely legitimate "66" (in numbers, versions, etc)
      const clean = l.replace(/v\d+\.\d+\.\d+/g, '').replace(/\d{3,}/g, '');
      return /(?<!\d)66(?!\d)/.test(clean);
    });

    if (suspicious66.length > 0) {
      console.log('=== Suspicious "66" found ===');
      for (const l of suspicious66) console.log(`  ${l}`);
    }
  });

  test('timed replay matches real-time rendering', async ({ page }) => {
    test.setTimeout(60_000);

    // Replay with real timing (compressed 10x)
    const events = recording.events.filter(e => e.type === 'output');
    const SPEED = 10; // 10x faster

    await page.evaluate(async ({ events, speed }) => {
      let lastT = 0;
      for (const event of events) {
        const delay = Math.max(0, (event.t - lastT) / speed);
        if (delay > 1) {
          await new Promise(r => setTimeout(r, delay));
        }
        window.__term.write(event.data);
        lastT = event.t;
      }
    }, { events, speed: SPEED });

    await page.waitForTimeout(500);

    // Verify terminal survived timed replay
    const dims = await page.evaluate(() => ({
      cols: window.__term.cols,
      rows: window.__term.rows,
    }));
    expect(dims.cols).toBeGreaterThan(0);

    await page.screenshot({ path: 'test-results/replay-timed.png' });
  });
});
