// test/memory-store.test.mjs
// Unit tests for the in-memory Store adapter. Node stdlib only (node:test + node:assert).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createMemoryStore } from '../src/infrastructure/memory-store.mjs';

test('save then get returns the stored url (round-trip)', () => {
  const store = createMemoryStore();
  store.save('aB3xY', 'https://example.com/');
  assert.equal(store.get('aB3xY'), 'https://example.com/');
});

test('has returns true for a stored code and false otherwise', () => {
  const store = createMemoryStore();
  store.save('code1', 'https://example.com/');
  assert.equal(store.has('code1'), true);
  assert.equal(store.has('missing'), false);
});

test('get returns undefined for an unknown code', () => {
  const store = createMemoryStore();
  assert.equal(store.get('nope'), undefined);
});

test('saving a duplicate code throws (no overwrite)', () => {
  const store = createMemoryStore();
  store.save('dup', 'https://first.example/');
  assert.throws(
    () => store.save('dup', 'https://second.example/'),
    /already exists/,
  );
  // original mapping is preserved after the rejected overwrite
  assert.equal(store.get('dup'), 'https://first.example/');
});

test('two store instances are isolated from each other', () => {
  const a = createMemoryStore();
  const b = createMemoryStore();
  a.save('shared', 'https://a.example/');

  assert.equal(a.has('shared'), true);
  assert.equal(b.has('shared'), false);
  assert.equal(b.get('shared'), undefined);

  // and B can reuse the same code independently without affecting A
  b.save('shared', 'https://b.example/');
  assert.equal(a.get('shared'), 'https://a.example/');
  assert.equal(b.get('shared'), 'https://b.example/');
});

test('size reflects the number of stored mappings', () => {
  const store = createMemoryStore();
  assert.equal(store.size(), 0);
  store.save('one', 'https://1.example/');
  store.save('two', 'https://2.example/');
  assert.equal(store.size(), 2);
});
