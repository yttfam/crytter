#!/usr/bin/env node
/**
 * WebSocket → PTY bridge + static file server for crytter.
 *
 * Usage: node server.mjs [port]
 *
 * Serves static files from project root and handles WebSocket
 * connections on /ws, spawning a shell for each.
 */

import { createServer } from 'http';
import { readFile, stat } from 'fs/promises';
import { join, extname } from 'path';
import { WebSocketServer } from 'ws';
import pty from 'node-pty';

const PORT = parseInt(process.argv[2] || '8080', 10);
const ROOT = new URL('.', import.meta.url).pathname;

const MIME = {
  '.html': 'text/html',
  '.js': 'application/javascript',
  '.wasm': 'application/wasm',
  '.css': 'text/css',
  '.json': 'application/json',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
};

// Static file server
const server = createServer(async (req, res) => {
  let path = join(ROOT, req.url === '/' ? '/www/index.html' : req.url);

  try {
    const s = await stat(path);
    if (s.isDirectory()) path = join(path, 'index.html');
    const data = await readFile(path);
    const ext = extname(path);
    res.writeHead(200, { 'Content-Type': MIME[ext] || 'application/octet-stream' });
    res.end(data);
  } catch {
    res.writeHead(404);
    res.end('not found');
  }
});

// WebSocket server on /ws path
const wss = new WebSocketServer({ server, path: '/ws' });

wss.on('connection', (ws) => {
  const shells = [process.env.SHELL, '/bin/zsh', '/bin/bash', '/bin/sh'].filter(Boolean);
  let ptyProcess;

  for (const shell of shells) {
    try {
      ptyProcess = pty.spawn(shell, [], {
        name: 'xterm-256color',
        cols: 80,
        rows: 24,
        cwd: process.env.HOME || '/',
        env: { ...process.env, TERM: 'xterm-256color' },
      });
      console.log(`[pty] spawned ${shell} (pid ${ptyProcess.pid})`);
      break;
    } catch (e) {
      console.error(`[pty] failed to spawn ${shell}: ${e.message}`);
    }
  }

  if (!ptyProcess) {
    console.error('[pty] all shells failed');
    ws.send('\x1b[31mFailed to spawn shell\x1b[0m\r\n');
    ws.close();
    return;
  }

  ptyProcess.onData((data) => {
    if (ws.readyState === 1) ws.send(data);
  });

  ptyProcess.onExit(({ exitCode }) => {
    console.log(`[pty] exited (code ${exitCode})`);
    if (ws.readyState === 1) ws.close();
  });

  ws.on('message', (msg) => {
    ptyProcess.write(msg.toString());
  });

  ws.on('close', () => {
    console.log('[ws] closed');
    ptyProcess.kill();
  });
});

server.listen(PORT, '0.0.0.0', () => {
  console.log(`crytter server on http://0.0.0.0:${PORT}`);
  console.log(`WebSocket PTY on ws://0.0.0.0:${PORT}/ws`);
});
