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

// Lifecycle-aware N/A categories. A criterion's category explains WHY an N/A is
// N/A, so a downstream consumer (forge-core's converge收敛层) can decide whether
// the absence is honest-and-permanent or a fixable tooling gap:
//   APPLICABLE   — a real check ran (PASS/FAIL): the criterion is fully verified.
//   INAPPLICABLE — the LANGUAGE/project has no such concept (no build step, no
//                  source language, the adapter declares no command). Honest at
//                  EVERY lifecycle: there is nothing to install that would make
//                  it applicable. Exempt anywhere.
//   NO_TOOL      — the concept applies but the TOOL is missing/unconfigured (a
//                  linter not installed, no advisory DB). A fixable gap: exempt
//                  while immature, but production must install the tool.
// HONESTY/FAIL-SAFE: only a verdict is APPLICABLE; an N/A defaults to NO_TOOL (the
// stricter category) unless its detail clearly matches an INAPPLICABLE phrase —
// so an unclassifiable N/A errs toward "go install the tool", never toward a free
// production exemption.
export const APPLICABLE = 'applicable';
export const INAPPLICABLE = 'inapplicable';
export const NO_TOOL = 'no_tool';

// inapplicableDetail patterns: the judge phrases that mean "this language/project
// has no such concept" (not "a tool is missing"). Kept as a small, explicit list
// matched against the result detail the existing probes already produce — the
// classifier reads ONLY status+detail, it does not re-judge. Conservative: a
// phrase must clearly denote absence-of-concept (no source / no build step /
// adapter declares no command / no apps) to be INAPPLICABLE; everything else N/A
// stays NO_TOOL.
const inapplicableDetail = [
  /\bno build step\b/i,
  /\bno source languages?\b/i,
  /\badapter has no\b/i,
  /\bno TS sources\b/i,
  /\bno example apps\b/i,
];

// categorize maps a finished result's {status,detail} onto its category. PURE (no
// I/O) so it is directly unit-testable. PASS/FAIL -> APPLICABLE (a check ran). An
// N/A -> INAPPLICABLE only when its detail matches an absence-of-concept phrase,
// else NO_TOOL (the fail-safe default — an unknown N/A is treated as a fixable
// tooling gap, never a free pass).
export function categorize(status, detail) {
  if (status !== NA) return APPLICABLE;
  return inapplicableDetail.some((re) => re.test(detail ?? '')) ? INAPPLICABLE : NO_TOOL;
}

// withCategory returns a copy of a result row annotated with its category. The
// category is an ADDITIVE field: decide()/LOAD_BEARING never read it (acceptance-
// layer behaviour is byte-for-byte unchanged), and any consumer reading the old
// {criterion,status,detail} shape ignores it. It exists so the --json bridge can
// carry the category to forge-core's lifecycle-aware exemption matrix.
export function withCategory(r) {
  return { ...r, category: categorize(r.status, r.detail) };
}

// splitCmd: a run-string ("eslint . --max-warnings=0") -> [cmd, argsArray] for
// run(). Whitespace split is sufficient for the adapter commands (no shell
// quoting/globs that need a shell); we deliberately do NOT use a shell so there
// is no injection surface from the adapter YAML.
export function splitCmd(cmd) {
  const [bin, ...args] = cmd.trim().split(/\s+/);
  return [bin, args];
}
