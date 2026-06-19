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
 * Advance from `fromId` until a derived code is free in the store, returning
 * that code and the id to resume from next time (one past the consumed id).
 * Module-level + pure-over-its-args so the factory body stays small and this
 * allocation rule is independently testable.
 * @param {{ has: (code: string) => boolean }} store
 * @param {number} fromId  first candidate id to try
 * @returns {{ code: string, nextId: number }}
 */
function allocFreeCode(store, fromId) {
  let id = fromId;
  let code = codeForId(id);
  while (store.has(code)) {
    id += 1;
    code = codeForId(id);
  }
  return { code, nextId: id + 1 }; // resume one past the id we just consumed
}

/**
 * Create a shortener service bound to an injected Store port.
 *
 * The service owns an internal monotonically increasing counter used to derive
 * deterministic short codes. On the (rare) event that a derived code is already
 * taken in the store, the counter advances and a fresh code is tried
 * (see {@link allocFreeCode}).
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
  let nextId = 0; // internal auto-increment counter -> next id to encode

  function shorten(url) {
    // One shared parse: canonicalizeUrl validates AND normalizes, returning the
    // canonical href or null. This guarantees the contracted `invalid url` Error
    // and never lets a raw WHATWG 'Invalid URL' from normalization escape.
    const normalized = canonicalizeUrl(url);
    if (normalized === null) {
      throw new Error(`shorten: invalid url: ${String(url)}`);
    }
    const alloc = allocFreeCode(store, nextId);
    nextId = alloc.nextId;
    store.save(alloc.code, normalized);
    return { code: alloc.code };
  }

  const resolve = (code) => store.get(code); // unknown code -> undefined

  return { shorten, resolve };
}
