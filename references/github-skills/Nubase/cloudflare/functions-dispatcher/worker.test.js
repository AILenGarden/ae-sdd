import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHmac, createHash } from 'node:crypto';
import worker from './worker.js';

const SECRET = 'test-dispatcher-secret';

// Signed payload lines (the contract shared with CloudflareEdgeFunctionExecutor.sign):
// requestId, projectRef, functionSlug, deploymentId, METHOD, rawPathSuffix,
// rawQuery, timestamp, sha256Hex(body)
function sign({ requestId, projectRef, functionSlug, deploymentId, method, path, query, timestamp, body }) {
  const hash = createHash('sha256').update(body).digest('hex');
  const payload = [requestId, projectRef, functionSlug, deploymentId, method.toUpperCase(), path, query, timestamp, hash].join('\n');
  return createHmac('sha256', SECRET).update(payload).digest('hex');
}

function signedRequest({
  base = 'https://dispatch.example.com',
  projectRef = 'app1',
  functionSlug = 'hello',
  deploymentId = 'nubase-app1-hello',
  method = 'POST',
  path = '/nested',
  query = 'x=1',
  body = '{"a":1}',
  timestamp = String(Math.floor(Date.now() / 1000)),
  tamper = {},
} = {}) {
  const signature = sign({
    requestId: 'req-1', projectRef, functionSlug, deploymentId, method, path, query, timestamp, body,
  });
  const url = `${base}/${projectRef}/${functionSlug}${path}${query ? `?${query}` : ''}`;
  const headers = {
    'content-type': 'application/json',
    'x-nubase-request-id': 'req-1',
    'x-nubase-project-ref': projectRef,
    'x-nubase-function-slug': functionSlug,
    'x-nubase-deployment-id': deploymentId,
    'x-nubase-timestamp': timestamp,
    'x-nubase-signature': signature,
    ...tamper,
  };
  return new Request(url, { method, headers, body: method === 'GET' ? undefined : body });
}

function makeEnv(onFetch) {
  return {
    NUBASE_DISPATCHER_SECRET: SECRET,
    NUBASE_DISPATCH: {
      get(deploymentId) {
        return {
          fetch(forwarded) {
            return onFetch(deploymentId, forwarded);
          },
        };
      },
    },
  };
}

test('valid signed request is routed and signature headers are stripped', async () => {
  let seen;
  const env = makeEnv((deploymentId, forwarded) => {
    seen = { deploymentId, forwarded };
    return new Response('ok', { status: 201 });
  });

  const res = await worker.fetch(signedRequest(), env);

  assert.equal(res.status, 201);
  assert.equal(seen.deploymentId, 'nubase-app1-hello');
  const url = new URL(seen.forwarded.url);
  assert.equal(url.pathname, '/nested');
  assert.equal(url.search, '?x=1');
  assert.equal(seen.forwarded.headers.get('x-nubase-signature'), null);
  assert.equal(seen.forwarded.headers.get('x-nubase-timestamp'), null);
  assert.equal(seen.forwarded.headers.get('x-nubase-deployment-id'), null);
  assert.equal(seen.forwarded.headers.get('x-nubase-project-ref'), 'app1');
  assert.equal(seen.forwarded.headers.get('x-nubase-function-slug'), 'hello');
  assert.equal(seen.forwarded.headers.get('x-nubase-request-id'), 'req-1');
});

test('replay with swapped deployment-id is rejected', async () => {
  let routed = false;
  const env = makeEnv(() => {
    routed = true;
    return new Response('ok');
  });

  // Signature was computed for nubase-app1-hello; attacker swaps the routing header.
  const res = await worker.fetch(
    signedRequest({ tamper: { 'x-nubase-deployment-id': 'nubase-victim-fn' } }),
    env
  );

  assert.equal(res.status, 401);
  assert.equal(routed, false);
  assert.deepEqual(await res.json(), { error: 'invalid_signature' });
});

test('expired timestamp is rejected', async () => {
  const env = makeEnv(() => new Response('ok'));
  const stale = String(Math.floor(Date.now() / 1000) - 301);
  const res = await worker.fetch(signedRequest({ timestamp: stale }), env);
  assert.equal(res.status, 401);
});

test('missing function context headers is a 400', async () => {
  const env = makeEnv(() => new Response('ok'));
  const req = signedRequest({ tamper: { 'x-nubase-deployment-id': '' } });
  const res = await worker.fetch(req, env);
  assert.equal(res.status, 400);
});

test('dispatcher mounted under a base path still verifies and forwards the suffix', async () => {
  let seen;
  const env = makeEnv((deploymentId, forwarded) => {
    seen = forwarded;
    return new Response('ok');
  });

  const res = await worker.fetch(signedRequest({ base: 'https://host.example.com/dispatch' }), env);

  assert.equal(res.status, 200);
  assert.equal(new URL(seen.url).pathname, '/nested');
});

test('percent-encoded path suffix verifies (raw path is signed)', async () => {
  let seen;
  const env = makeEnv((deploymentId, forwarded) => {
    seen = forwarded;
    return new Response('ok');
  });

  const res = await worker.fetch(signedRequest({ path: '/a%20b/c', query: '' }), env);

  assert.equal(res.status, 200);
  assert.equal(new URL(seen.url).pathname, '/a%20b/c');
});

test('slug-only invocation forwards root path', async () => {
  let seen;
  const env = makeEnv((deploymentId, forwarded) => {
    seen = forwarded;
    return new Response('ok');
  });

  const res = await worker.fetch(signedRequest({ path: '', query: '' }), env);

  assert.equal(res.status, 200);
  assert.equal(new URL(seen.url).pathname, '/');
});

test('path not containing the signed project/function segments is a 400', async () => {
  const env = makeEnv(() => new Response('ok'));
  const req = signedRequest({ tamper: { 'x-nubase-project-ref': 'other' } });
  const res = await worker.fetch(req, env);
  assert.equal(res.status, 400);
  assert.deepEqual(await res.json(), { error: 'path_mismatch' });
});
