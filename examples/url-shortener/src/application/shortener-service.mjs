// application/shortener-service.mjs
// Use-case orchestration for the URL shortener.
//
// Clean Architecture: this layer depends only on the `domain` (pure functions)
// and on a Store *port* that is INJECTED via `createService(store)`. It must NOT
// import `infrastructure` — the concrete store (e.g. memory-store) is wired in by
// the caller/composition root. Dependencies point inward only.
//
// Store port contract (see SPEC.md "契约"):
//   save(code, url): void      — store mapping; throws if `code` already exists
//   get(code): string|undefined — stored url, or undefined when absent
//   has(code): boolean          — whether `code` is already taken

import { codeForId } from '../domain/shortcode.mjs';
import { canonicalizeUrl } from '../domain/url.mjs';

/**
 * Assert that `store` satisfies the Store port shape, else throw.
 * @param {unknown} store
 * @returns {void}
 */
function assertStorePort(store) {
  const ok = store && typeof store.save === 'function'
    && typeof store.get === 'function' && typeof store.has === 'function';
  if (!ok) {
    throw new Error('createService: a Store port { save, get, has } is required');
  }
}

/**
 * Create a shortener service bound to an injected Store port.
 *
 * The service owns an internal monotonically increasing counter used to derive
 * deterministic short codes. On the (rare) event that a derived code is already
 * taken in the store, the counter advances and a fresh code is tried.
 *
 * @param {{
 *   save: (code: string, url: string) => void,
 *   get: (code: string) => (string | undefined),
 *   has: (code: string) => boolean,
 * }} store  Store port implementation (injected; never imported here).
 * @returns {{
 *   shorten: (url: string) => { code: string },
 *   resolve: (code: string) => (string | undefined),
 * }}
 */
export function createService(store) {
  assertStorePort(store);

  // Internal auto-increment counter -> next id to encode. Starts at 0.
  let nextId = 0;

  /**
   * Advance the counter until it yields a code not already taken in the store,
   * then return that code (without consuming further ids).
   * @returns {string} a base62 code free in the store
   */
  function nextFreeCode() {
    let code = codeForId(nextId);
    while (store.has(code)) {
      nextId += 1;
      code = codeForId(nextId);
    }
    nextId += 1; // consume the id we just allocated
    return code;
  }

  /**
   * Validate, normalize and persist a URL, returning its assigned short code.
   * @param {string} url  candidate long URL (http/https)
   * @returns {{ code: string }}
   * @throws {Error} when `url` is not a valid http/https URL
   */
  function shorten(url) {
    // One shared parse: canonicalizeUrl validates AND normalizes, returning the
    // canonical href or null. This guarantees the contracted `invalid url` Error
    // and never lets a raw WHATWG 'Invalid URL' from normalization escape.
    const normalized = canonicalizeUrl(url);
    if (normalized === null) {
      throw new Error(`shorten: invalid url: ${String(url)}`);
    }
    const code = nextFreeCode();
    store.save(code, normalized);
    return { code };
  }

  /**
   * Resolve a short code back to its stored URL.
   * @param {string} code
   * @returns {string | undefined} the URL, or undefined for an unknown code
   */
  function resolve(code) {
    return store.get(code);
  }

  return { shorten, resolve };
}
