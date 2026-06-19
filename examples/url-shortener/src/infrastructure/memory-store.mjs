// infrastructure/memory-store.mjs
// In-memory adapter implementing the application's Store port (backed by a Map).
//
// Clean Architecture note: this is an infrastructure adapter injected into the
// application service via the Store port. It depends on nothing but the Node
// standard library (Map) and imports no other layer (no domain/application/interface).
//
// Store port (see SPEC.md "契约"):
//   save(code, url): void      — store mapping; throw Error if `code` already exists
//   get(code): string|undefined — return stored url, or undefined if absent
//   has(code): boolean          — whether `code` is stored
//   size(): number              — current entry count (test convenience)

/**
 * Create an isolated in-memory Store.
 *
 * Each call returns an independent store backed by its own Map, so instances do
 * not share state — convenient for dependency injection and unit testing.
 *
 * @returns {{
 *   save: (code: string, url: string) => void,
 *   get: (code: string) => (string | undefined),
 *   has: (code: string) => boolean,
 *   size: () => number,
 * }}
 */
export function createMemoryStore() {
  /** @type {Map<string, string>} */
  const map = new Map();

  /**
   * Store a code → url mapping.
   * Refuses to overwrite an existing code (prevents silent collision loss).
   * @param {string} code
   * @param {string} url
   * @returns {void}
   */
  function save(code, url) {
    if (map.has(code)) {
      throw new Error(`memory-store: code already exists: ${code}`);
    }
    map.set(code, url);
  }

  /**
   * Resolve a code to its url, or undefined when not present.
   * @param {string} code
   * @returns {string | undefined}
   */
  function get(code) {
    return map.get(code);
  }

  /**
   * Whether a code is currently stored.
   * @param {string} code
   * @returns {boolean}
   */
  function has(code) {
    return map.has(code);
  }

  /**
   * Number of stored mappings (handy for assertions in tests).
   * @returns {number}
   */
  function size() {
    return map.size;
  }

  return { save, get, has, size };
}
