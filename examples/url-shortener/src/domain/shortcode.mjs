// domain/shortcode.mjs — pure functions, zero dependencies (Node stdlib only).
// Deterministic base62 short-code encoding. Knows nothing of the outside world.

const ALPHABET = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';
const BASE = ALPHABET.length; // 62
const PAD_CHAR = ALPHABET[0]; // '0'

const CODE_RE = /^[0-9A-Za-z]+$/;

/**
 * Deterministically encode a non-negative integer `id` to a base62 string,
 * left-padded with the base62 zero ('0') to at least `len` characters.
 * Larger ids naturally exceed `len`.
 *
 * @param {number} id  non-negative integer
 * @param {number} [len=5]  minimum output length
 * @returns {string} base62 short code
 */
export function codeForId(id, len = 5) {
  if (!Number.isInteger(id) || id < 0) {
    throw new Error(`codeForId: id must be a non-negative integer, got ${id}`);
  }
  if (!Number.isInteger(len) || len < 0) {
    throw new Error(`codeForId: len must be a non-negative integer, got ${len}`);
  }

  let n = id;
  let out = '';
  // Build from least-significant digit; id===0 yields a single PAD_CHAR.
  do {
    out = ALPHABET[n % BASE] + out;
    n = Math.floor(n / BASE);
  } while (n > 0);

  return out.padStart(len, PAD_CHAR);
}

/**
 * True iff `s` is a non-empty string containing only base62 characters.
 *
 * @param {unknown} s
 * @returns {boolean}
 */
export function isValidCode(s) {
  return typeof s === 'string' && s.length > 0 && CODE_RE.test(s);
}
