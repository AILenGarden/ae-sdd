import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { authorize, buildAuthorizeUrl, parseAuthorizeArgs } from '../src/authorize.js';

test('parseAuthorizeArgs uses production defaults', () => {
  const options = parseAuthorizeArgs([], {});

  assert.equal(options.nubaseUrl, 'https://nubase.ai');
  assert.equal(options.studioUrl, 'https://nubase.ai/studio');
  assert.equal(options.openBrowser, true);
  assert.equal(options.timeoutMs, 300000);
  assert.equal(options.promptOnly, false);
  assert.match(options.configPath, /\/\.nubase\/config\.json$/);
});

test('parseAuthorizeArgs reads explicit URLs and disables browser open', () => {
  const options = parseAuthorizeArgs([
    '--nubase-url',
    'https://api.example.com/',
    '--studio-url',
    'https://studio.example.com/',
    '--agent-id',
    'codex',
    '--config',
    '/tmp/nubase-config.json',
    '--timeout-seconds',
    '10',
    '--no-open',
  ]);

  assert.equal(options.nubaseUrl, 'https://api.example.com');
  assert.equal(options.studioUrl, 'https://studio.example.com');
  assert.equal(options.agentId, 'codex');
  assert.equal(options.configPath, '/tmp/nubase-config.json');
  assert.equal(options.timeoutMs, 10000);
  assert.equal(options.openBrowser, false);
});

test('parseAuthorizeArgs supports prompt-only authorization URL mode', () => {
  const options = parseAuthorizeArgs(['--prompt-only']);

  assert.equal(options.openBrowser, false);
  assert.equal(options.promptOnly, true);
});

test('buildAuthorizeUrl preserves a Studio base path', () => {
  const url = buildAuthorizeUrl({
    studioUrl: 'https://nubase.ai/studio',
    nubaseUrl: 'https://nubase.ai',
    callbackUrl: 'http://127.0.0.1:12345/callback',
    sessionId: 'session-1',
    state: 'state-1',
    agentId: 'codex',
  });

  assert.equal(url.pathname, '/studio/cli/authorize');
  assert.equal(url.searchParams.get('callback'), 'http://127.0.0.1:12345/callback');
  assert.equal(url.searchParams.get('nubase_url'), 'https://nubase.ai');
  assert.equal(url.searchParams.get('agent_id'), 'codex');
});

test('authorize callback responds to CORS preflight and POST', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'nubase-cli-auth-'));
  const originalConfig = process.env.NUBASE_CONFIG;
  process.env.NUBASE_CONFIG = path.join(dir, 'config.json');

  try {
    let authorizeUrlText = '';
    const originalError = console.error;
    console.error = (...args: unknown[]) => {
      const text = args.join(' ');
      if (text.startsWith('https://studio.example/cli/authorize?')) {
        authorizeUrlText = text;
      }
    };

    const authorization = authorize({
      nubaseUrl: 'https://api.example',
      studioUrl: 'https://studio.example',
      openBrowser: false,
      promptOnly: true,
      timeoutMs: 5000,
      configPath: process.env.NUBASE_CONFIG,
    }).finally(() => {
      console.error = originalError;
    });

    await waitFor(() => authorizeUrlText);
    const authorizeUrl = new URL(authorizeUrlText);
    const callback = authorizeUrl.searchParams.get('callback');
    const state = authorizeUrl.searchParams.get('state');
    assert.ok(callback);
    assert.ok(state);

    const preflight = await fetch(callback, {
      method: 'OPTIONS',
      headers: {
        Origin: 'https://studio.example',
        'Access-Control-Request-Method': 'POST',
        'Access-Control-Request-Headers': 'content-type',
      },
    });
    assert.equal(preflight.status, 204);
    assert.equal(preflight.headers.get('access-control-allow-origin'), '*');
    assert.match(preflight.headers.get('access-control-allow-methods') ?? '', /POST/);
    assert.match(preflight.headers.get('access-control-allow-headers') ?? '', /Content-Type/i);
    assert.equal(preflight.headers.get('access-control-allow-private-network'), 'true');

    const post = await fetch(callback, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Origin: 'https://studio.example' },
      body: JSON.stringify({ state, projectKey: 'project-key' }),
    });
    assert.equal(post.status, 200);
    assert.equal(post.headers.get('access-control-allow-origin'), '*');

    const saved = await authorization;
    assert.equal(saved.projectKey, 'project-key');
  } finally {
    if (originalConfig === undefined) {
      delete process.env.NUBASE_CONFIG;
    } else {
      process.env.NUBASE_CONFIG = originalConfig;
    }
    await rm(dir, { recursive: true, force: true });
  }
});

async function waitFor<T>(read: () => T, timeoutMs = 1000): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = read();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  throw new Error('Timed out waiting for value');
}
