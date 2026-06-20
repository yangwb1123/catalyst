// ForgeOS acceptance KERNEL — the shared, dependency-free primitives every
// acceptance probe is built on. Split out of acceptance.mjs (which had grown to
// the 500-line cap, packing a shared runner kernel + 7 probe families + the
// app-test subsystem + orchestration + verdict + render into one file — a
// single-responsibility violation a fresh reviewer flagged).
//
// This module is the BOTTOM of the acceptance dependency graph: it imports ONLY
// node: builtins and is imported BY acceptance-quality.mjs and acceptance.mjs —
// never the reverse. Keeping it internal-dependency-free is what makes the split
// acyclic: kernel  <-  quality  <-  acceptance, one direction, no back-edges.
//
// Behaviour is byte-for-byte identical to the pre-split definitions; this is a
// pure code move (the constants, run(), result(), splitCmd() are unchanged).
import { spawnSync } from 'node:child_process';
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

export const HARNESS_DIR = dirname(fileURLToPath(import.meta.url));
export const ROOT = dirname(HARNESS_DIR);

// Verdict statuses for a single criterion.
export const PASS = 'PASS';
export const FAIL = 'FAIL';
export const NA = 'N-A';

// --- low-level command runner ------------------------------------------------

// Run a command; return {ok, code, out} where ok === exit 0. Centralised so every
// probe judges success the same way (exit-code === 0). `extraEnv` overlays env
// vars (e.g. FORGE_ACCEPT_INNER) on the scrubbed parent env. `cwd` defaults to the
// repo root; a probe overrides it for a command whose paths are relative to a
// subdir (e.g. an adapter `go test ./...` that must run from the app's module dir).
export function run(cmd, args, extraEnv = {}, cwd = ROOT) {
  // Scrub NODE_TEST_CONTEXT so a nested `node --test` (when acceptance.mjs is
  // itself spawned from under `node --test`, e.g. test_acceptance.mjs) runs as a
  // fresh top-level run and prints its TAP summary to stdout, rather than
  // switching to child-reporter mode (which emits no `# tests N` and would make
  // the app-test count read 0 → a false "no app tests discovered").
  const env = { ...process.env, ...extraEnv };
  delete env.NODE_TEST_CONTEXT;
  const res = spawnSync(cmd, args, { cwd, encoding: 'utf8', env });
  if (res.error) return { ok: false, code: null, out: String(res.error.message) };
  const out = `${res.stdout || ''}${res.stderr || ''}`.trim();
  return { ok: res.status === 0, code: res.status, out };
}

export function result(criterion, status, detail) {
  return { criterion, status, detail };
}

// splitCmd: a run-string ("eslint . --max-warnings=0") -> [cmd, argsArray] for
// run(). Whitespace split is sufficient for the adapter commands (no shell
// quoting/globs that need a shell); we deliberately do NOT use a shell so there
// is no injection surface from the adapter YAML.
export function splitCmd(cmd) {
  const [bin, ...args] = cmd.trim().split(/\s+/);
  return [bin, args];
}
