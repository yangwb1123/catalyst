// test/shortener-service.test.mjs
// Integration tests for the application service wired to the REAL memory store
// (no mocks) — exercises the Store port contract end to end. Node stdlib only.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createService } from '../src/application/shortener-service.mjs';
import { createMemoryStore } from '../src/infrastructure/memory-store.mjs';

test('shorten then resolve returns the normalized url (round-trip)', () => {
  const svc = createService(createMemoryStore());
  const { code } = svc.shorten('https://example.com/path');
  assert.equal(typeof code, 'string');
  assert.ok(code.length > 0);
  assert.equal(svc.resolve(code), 'https://example.com/path');
});

test('shorten throws on an invalid (non-http) url', () => {
  const svc = createService(createMemoryStore());
  assert.throws(() => svc.shorten('javascript:alert(1)'), /invalid url/);
  assert.throws(() => svc.shorten('not a url at all !!!'), /invalid url/);
  assert.throws(() => svc.shorten(''), /invalid url/);
});

test('different urls get different codes', () => {
  const svc = createService(createMemoryStore());
  const a = svc.shorten('https://a.example/').code;
  const b = svc.shorten('https://b.example/').code;
  const c = svc.shorten('https://c.example/').code;
  assert.notEqual(a, b);
  assert.notEqual(b, c);
  assert.notEqual(a, c);
});

test('resolve returns undefined for an unknown code', () => {
  const svc = createService(createMemoryStore());
  assert.equal(svc.resolve('zzzzz'), undefined);
});

test('shorten canonicalizes a valid url (adds the trailing slash)', () => {
  // A single `canonicalizeUrl` parse both validates (http/https only) and
  // returns the canonical href — here a trailing slash on a bare-origin URL.
  const svc = createService(createMemoryStore());
  const { code } = svc.shorten('https://example.org');
  assert.equal(svc.resolve(code), 'https://example.org/');
});

test('shorten rejects scheme-only inputs without leaking a raw URL error', () => {
  // Regression: `http:example.com` / `https:foo` once passed isValidUrl but
  // made the old normalizeUrl throw a raw WHATWG 'Invalid URL'. The shared
  // single-parse path must reject them with the contracted `invalid url` Error.
  const svc = createService(createMemoryStore());
  assert.throws(() => svc.shorten('http:example.com'), /invalid url/);
  assert.throws(() => svc.shorten('https:foo'), /invalid url/);
});

test('shorten rejects a scheme-less bare host (invalid url)', () => {
  // Per the domain contract `isValidUrl('example.org') === false`, so the
  // service rejects it rather than guessing a scheme.
  const svc = createService(createMemoryStore());
  assert.throws(() => svc.shorten('example.org'), /invalid url/);
});

test('the same url shortened twice yields two distinct codes', () => {
  // The service assigns a fresh code per call (counter-based); it does not dedupe.
  const svc = createService(createMemoryStore());
  const first = svc.shorten('https://dup.example/').code;
  const second = svc.shorten('https://dup.example/').code;
  assert.notEqual(first, second);
  assert.equal(svc.resolve(first), 'https://dup.example/');
  assert.equal(svc.resolve(second), 'https://dup.example/');
});

test('skips a code already taken in the store (collision retry)', () => {
  // Pre-occupy the very first code the counter would produce ("00000"),
  // forcing the service to advance to the next free code.
  const store = createMemoryStore();
  store.save('00000', 'https://pre.example/');
  const svc = createService(store);
  const { code } = svc.shorten('https://fresh.example/');
  assert.notEqual(code, '00000');
  assert.equal(svc.resolve(code), 'https://fresh.example/');
  assert.equal(svc.resolve('00000'), 'https://pre.example/');
});

test('createService requires a valid Store port', () => {
  assert.throws(() => createService(null), /Store port/);
  assert.throws(() => createService({}), /Store port/);
});

test('createService rejects a store missing any single port method', () => {
  // Each partial store omits exactly one required method; all must be rejected.
  const noop = () => {};
  assert.throws(() => createService({ get: noop, has: noop }), /Store port/); // no save
  assert.throws(() => createService({ save: noop, has: noop }), /Store port/); // no get
  assert.throws(() => createService({ save: noop, get: noop }), /Store port/); // no has
});
