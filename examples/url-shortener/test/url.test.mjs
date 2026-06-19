// Unit tests for domain/url.mjs — node:test + node:assert, no deps.
import test from 'node:test';
import assert from 'node:assert/strict';
import { isValidUrl, normalizeUrl } from '../src/domain/url.mjs';

test('isValidUrl is true for http and https URLs', () => {
  assert.equal(isValidUrl('http://example.com'), true);
  assert.equal(isValidUrl('https://example.com'), true);
  assert.equal(isValidUrl('https://example.com/path?q=1#frag'), true);
  assert.equal(isValidUrl('http://localhost:8080'), true);
  assert.equal(isValidUrl('HTTPS://EXAMPLE.COM'), true); // scheme is case-insensitive
});

test('isValidUrl is false for non-http(s) protocols', () => {
  assert.equal(isValidUrl('ftp://example.com'), false);
  assert.equal(isValidUrl('javascript:alert(1)'), false);
  assert.equal(isValidUrl('mailto:a@b.com'), false);
  assert.equal(isValidUrl('file:///etc/passwd'), false);
  assert.equal(isValidUrl('data:text/plain,hi'), false);
});

test('isValidUrl is false for unparseable or empty input', () => {
  assert.equal(isValidUrl('not a url'), false);
  assert.equal(isValidUrl('example.com'), false); // no protocol => not absolute
  assert.equal(isValidUrl(''), false);
  assert.equal(isValidUrl('   '), false);
  assert.equal(isValidUrl(null), false);
  assert.equal(isValidUrl(undefined), false);
  assert.equal(isValidUrl(42), false);
});

test('normalizeUrl prepends https:// when no protocol is present', () => {
  assert.equal(normalizeUrl('example.com'), 'https://example.com/');
  assert.equal(normalizeUrl('example.com/path'), 'https://example.com/path');
});

test('normalizeUrl preserves an explicit protocol', () => {
  assert.equal(normalizeUrl('http://example.com'), 'http://example.com/');
  assert.equal(normalizeUrl('https://example.com/x'), 'https://example.com/x');
});

test('normalizeUrl trims surrounding whitespace', () => {
  assert.equal(normalizeUrl('  example.com  '), 'https://example.com/');
  assert.equal(normalizeUrl('\thttp://example.com\n'), 'http://example.com/');
});

test('normalizeUrl returns a canonical href', () => {
  // WHATWG URL canonicalization: host is lowercased, default path added.
  assert.equal(normalizeUrl('HTTP://Example.COM'), 'http://example.com/');
  assert.equal(typeof normalizeUrl('example.com'), 'string');
});

test('normalizeUrl throws on non-string input', () => {
  assert.throws(() => normalizeUrl(null));
  assert.throws(() => normalizeUrl(123));
});
