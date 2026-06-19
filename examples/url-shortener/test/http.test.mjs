// test/http.test.mjs
// End-to-end HTTP tests over a real listening server, driven with the global
// `fetch`. Wires interface -> application -> infrastructure (memory store).
// Node stdlib only (node:test + node:assert + global fetch).
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from '../src/interface/http-server.mjs';
import { createService } from '../src/application/shortener-service.mjs';
import { createMemoryStore } from '../src/infrastructure/memory-store.mjs';

let server;
let base;

before(async () => {
  server = createServer(createService(createMemoryStore()));
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const { port } = server.address();
  base = `http://127.0.0.1:${port}`;
});

after(async () => {
  await new Promise((resolve) => server.close(resolve));
});

test('POST /shorten returns 201 with code and short path', async () => {
  const res = await fetch(`${base}/shorten`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ url: 'https://example.com/page' }),
  });
  assert.equal(res.status, 201);
  const body = await res.json();
  assert.equal(typeof body.code, 'string');
  assert.ok(body.code.length > 0);
  assert.equal(body.short, `/${body.code}`);
});

test('GET /:code returns 302 with the original url in Location', async () => {
  const target = 'https://redirect.example/deep/link';
  const post = await fetch(`${base}/shorten`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ url: target }),
  });
  const { code } = await post.json();

  // `redirect: 'manual'` so fetch surfaces the 302 instead of following it.
  const res = await fetch(`${base}/${code}`, { redirect: 'manual' });
  assert.equal(res.status, 302);
  assert.equal(res.headers.get('location'), target);
});

test('GET /:code returns 404 for an unknown code', async () => {
  const res = await fetch(`${base}/nope404`, { redirect: 'manual' });
  assert.equal(res.status, 404);
});

test('POST /shorten returns 400 for an invalid url', async () => {
  const res = await fetch(`${base}/shorten`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ url: 'ftp://not-allowed.example/' }),
  });
  assert.equal(res.status, 400);
  const body = await res.json();
  assert.equal(typeof body.error, 'string');
});

test('POST /shorten returns 400 for a malformed JSON body', async () => {
  const res = await fetch(`${base}/shorten`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: '{ this is not json',
  });
  assert.equal(res.status, 400);
});

test('POST /shorten returns 400 when the url field is missing', async () => {
  const res = await fetch(`${base}/shorten`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ notUrl: 'https://example.com/' }),
  });
  assert.equal(res.status, 400);
});

test('POST /shorten returns 413 for an oversized body (> 1 MiB)', async () => {
  // Just over the 1 MiB cap: the server must respond gracefully (413), not
  // reset the connection. Build a syntactically-valid JSON body so the only
  // thing that can trip is the size guard.
  const filler = 'x'.repeat((1 << 20) + 1024);
  const body = JSON.stringify({ url: `https://example.com/${filler}` });
  const res = await fetch(`${base}/shorten`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body,
  });
  assert.equal(res.status, 413);
  const payload = await res.json();
  assert.equal(typeof payload.error, 'string');
});

// Non-object / non-string-url JSON bodies must all be rejected with 400 by the
// body parser (no service call, no crash). One case per shape.
for (const bad of ['null', '42', '[1,2,3]', '{"url":5}']) {
  test(`POST /shorten returns 400 for body = ${bad}`, async () => {
    const res = await fetch(`${base}/shorten`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: bad,
    });
    assert.equal(res.status, 400);
    const payload = await res.json();
    assert.equal(typeof payload.error, 'string');
  });
}

test('GET // (malformed target) returns 400 and the server stays up', async () => {
  // A request target that makes `new URL(req.url, base)` throw must not crash
  // the whole server: it returns 400, and a subsequent valid request succeeds.
  const bad = await fetch(`${base}//`, { redirect: 'manual' });
  assert.equal(bad.status, 400);

  // Server is still serving: a normal POST /shorten must work afterwards.
  const ok = await fetch(`${base}/shorten`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ url: 'https://still-up.example/' }),
  });
  assert.equal(ok.status, 201);
});

test('a service throwing synchronously yields a controlled 400 (no crash)', async () => {
  // The createServer dispatch must catch a synchronous throw from a handler and
  // turn it into 400 rather than letting it escape and crash the process.
  const boom = {
    shorten() { throw new Error('boom'); },
    resolve() { return undefined; },
  };
  const srv = createServer(boom);
  await new Promise((resolve) => srv.listen(0, '127.0.0.1', resolve));
  try {
    const { port } = srv.address();
    const res = await fetch(`http://127.0.0.1:${port}/shorten`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ url: 'https://example.com/' }),
    });
    assert.equal(res.status, 400);
    const payload = await res.json();
    assert.equal(typeof payload.error, 'string');
  } finally {
    await new Promise((resolve) => srv.close(resolve));
  }
});
