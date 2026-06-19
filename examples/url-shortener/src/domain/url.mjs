// domain/url.mjs — pure functions, zero dependencies (Node stdlib only).
// URL validation/normalization. Uses the platform-global WHATWG `URL`.

const ALLOWED_PROTOCOLS = new Set(['http:', 'https:']);
// Matches a leading "scheme://" (RFC 3986 scheme chars) to detect an
// explicit protocol prefix before normalization.
const HAS_PROTOCOL_RE = /^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//;

/**
 * True iff `s` parses as an absolute URL with an http: or https: protocol.
 * Any parse failure or other protocol (ftp:, javascript:, ...) returns false.
 *
 * @param {unknown} s
 * @returns {boolean}
 */
export function isValidUrl(s) {
  if (typeof s !== 'string' || s.length === 0) return false;
  let parsed;
  try {
    parsed = new URL(s);
  } catch {
    return false;
  }
  return ALLOWED_PROTOCOLS.has(parsed.protocol);
}

/**
 * Normalize a URL string: trim surrounding whitespace, prepend `https://`
 * when no protocol prefix is present, and return the canonical href.
 *
 * @param {string} s
 * @returns {string} canonical URL href
 */
export function normalizeUrl(s) {
  if (typeof s !== 'string') {
    throw new Error(`normalizeUrl: expected string, got ${typeof s}`);
  }
  const trimmed = s.trim();
  const withProtocol = HAS_PROTOCOL_RE.test(trimmed)
    ? trimmed
    : `https://${trimmed}`;
  return new URL(withProtocol).href;
}

/**
 * Single-parse canonicalize for the application layer: validate AND normalize
 * via one WHATWG parse so the two can never diverge. Returns the canonical href
 * for an absolute http/https URL with an explicit `scheme://` authority, else
 * `null` (no throw, no scheme-guessing).
 *
 * Requiring the `://` authority closes the gap where `isValidUrl('http:foo')`
 * accepted an input that the old `normalizeUrl` then re-prefixed into an invalid
 * string and threw a raw WHATWG 'Invalid URL': such inputs now return null and
 * the service raises its contracted `invalid url` Error instead.
 *
 * @param {unknown} s
 * @returns {string | null} canonical href, or null when not an http/https URL
 */
export function canonicalizeUrl(s) {
  if (typeof s !== 'string' || s.length === 0) return null;
  if (!HAS_PROTOCOL_RE.test(s)) return null;
  let parsed;
  try {
    parsed = new URL(s);
  } catch {
    return null;
  }
  return ALLOWED_PROTOCOLS.has(parsed.protocol) ? parsed.href : null;
}
