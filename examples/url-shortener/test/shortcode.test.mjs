// Unit tests for domain/shortcode.mjs — node:test + node:assert, no deps.
import test from 'node:test';
import assert from 'node:assert/strict';
import { codeForId, isValidCode } from '../src/domain/shortcode.mjs';

const BASE62_RE = /^[0-9A-Za-z]+$/;

test('codeForId is deterministic: same id yields same code', () => {
  assert.equal(codeForId(12345), codeForId(12345));
  assert.equal(codeForId(0), codeForId(0));
  assert.equal(codeForId(61, 5), codeForId(61, 5));
});

test('codeForId maps distinct ids to distinct codes', () => {
  const seen = new Map();
  for (let id = 0; id < 1000; id++) {
    const code = codeForId(id);
    assert.equal(seen.has(code), false, `collision at id=${id}: ${code}`);
    seen.set(code, id);
  }
});

test('codeForId left-pads to at least len with base62 zero', () => {
  assert.equal(codeForId(0).length, 5);
  assert.equal(codeForId(0), '00000');
  assert.equal(codeForId(1), '00001');
  assert.equal(codeForId(61), '0000z'); // last single-digit base62 char
  assert.equal(codeForId(7, 3), '007');
  assert.equal(codeForId(0, 1), '0');
  assert.equal(codeForId(5, 0), '5'); // len=0 => no padding
});

test('codeForId can exceed len for large ids', () => {
  const big = codeForId(62 ** 6); // needs 7 base62 digits
  assert.ok(big.length > 5, `expected >5 chars, got "${big}" (${big.length})`);
});

test('codeForId output is pure base62', () => {
  for (const id of [0, 1, 61, 62, 1000, 999999, 62 ** 5]) {
    assert.match(codeForId(id), BASE62_RE);
  }
});

test('codeForId round-trips: decoding the code recovers the id', () => {
  const alphabet = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';
  const decode = (code) =>
    [...code].reduce((acc, ch) => acc * 62 + alphabet.indexOf(ch), 0);
  for (const id of [0, 1, 61, 62, 3843, 100000, 62 ** 4 + 7]) {
    assert.equal(decode(codeForId(id)), id, `round-trip failed for ${id}`);
  }
});

test('codeForId rejects invalid ids', () => {
  assert.throws(() => codeForId(-1));
  assert.throws(() => codeForId(1.5));
  assert.throws(() => codeForId('7'));
  assert.throws(() => codeForId(10, -1));
});

test('isValidCode is true for non-empty base62 strings', () => {
  assert.equal(isValidCode('aB3xY'), true);
  assert.equal(isValidCode('0'), true);
  assert.equal(isValidCode('zZ09'), true);
  assert.equal(isValidCode(codeForId(424242)), true);
});

test('isValidCode is false for empty or non-base62 input', () => {
  assert.equal(isValidCode(''), false);
  assert.equal(isValidCode('ab-cd'), false);
  assert.equal(isValidCode('ab cd'), false);
  assert.equal(isValidCode('héllo'), false);
  assert.equal(isValidCode('a/b'), false);
  assert.equal(isValidCode(null), false);
  assert.equal(isValidCode(12345), false);
  assert.equal(isValidCode(undefined), false);
});
