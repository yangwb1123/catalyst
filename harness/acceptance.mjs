#!/usr/bin/env node
// ForgeOS acceptance gate — the executable Stop gate for the Evaluation layer.
// Maps `.agent/eval/acceptance.schema.yml` criteria onto THIS repo's existing
// out-of-band checks and renders an ACCEPTED / REJECTED verdict.
//
// Honesty first: a criterion is only PASS/FAIL when a real check backs it.
// Criteria with no executable check in this repo (coverage / lint / typecheck
// / build / security_findings) are reported N/A with a reason — never faked
// into a pass, and N/A is never counted as satisfied.
//
// Design: one criterion == one probe function (shells its command, judges the
// exit code); a runner collects results; `decide()` is a PURE function over the
// results array so the verdict is unit-testable without spawning anything.
//
// CLI: `node harness/acceptance.mjs`         ->  exit 0 ACCEPTED · exit 1 REJECTED.
//      `node harness/acceptance.mjs --json`  ->  per-criterion JSON to stdout.
import { spawnSync } from 'node:child_process';
import { readdirSync, statSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HARNESS_DIR = dirname(fileURLToPath(import.meta.url));
const ROOT = dirname(HARNESS_DIR);

// Verdict statuses for a single criterion.
export const PASS = 'PASS';
export const FAIL = 'FAIL';
export const NA = 'N-A';

// --- low-level command runner ------------------------------------------------

// Run a command from the repo root; return {ok, code, out} where ok === exit 0.
// Centralised so every probe judges success the same way (exit-code === 0).
// `extraEnv` overlays additional env vars (e.g. FORGE_ACCEPT_INNER for the
// nested-suite self-spawn guard) on top of the scrubbed parent environment.
function run(cmd, args, extraEnv = {}) {
  // Scrub NODE_TEST_CONTEXT so a nested `node --test` (when acceptance.mjs is
  // itself spawned from under `node --test`, e.g. test_acceptance.mjs) runs as a
  // fresh top-level run and prints its TAP summary to stdout, rather than
  // switching to child-reporter mode (which emits no `# tests N` and would make
  // the app-test count read 0 → a false "no app tests discovered").
  const env = { ...process.env, ...extraEnv };
  delete env.NODE_TEST_CONTEXT;
  const res = spawnSync(cmd, args, { cwd: ROOT, encoding: 'utf8', env });
  if (res.error) return { ok: false, code: null, out: String(res.error.message) };
  const out = `${res.stdout || ''}${res.stderr || ''}`.trim();
  return { ok: res.status === 0, code: res.status, out };
}

function result(criterion, status, detail) {
  return { criterion, status, detail };
}

// --- per-criterion probes (one criterion == one function) --------------------

// complexity_violations == 0  <-  structural gate (per-file line cap + root count).
export function probeComplexity() {
  const r = run('node', [join(HARNESS_DIR, 'gate.mjs')]);
  return result(
    'complexity_violations',
    r.ok ? PASS : FAIL,
    r.ok ? 'gate.mjs: structural caps clean' : `gate.mjs exit ${r.code}`,
  );
}

// arch_violations == 0  <-  governance integrity (broken agent/skill refs, tiers).
export function probeArch() {
  const r = run('python3', [join(HARNESS_DIR, 'check.py')]);
  return result(
    'arch_violations',
    r.ok ? PASS : FAIL,
    r.ok ? 'check.py: governance integrity clean' : `check.py exit ${r.code}`,
  );
}

// test_pass == true  <-  ALL committed harness suites must be green.
// Self-governance: the harness must run its OWN tests, not a curated subset, or
// a regression in (e.g.) acceptance.mjs's verdict logic — OR a silently-green
// arch-check (the enforcer faking a pass) — could ship unnoticed.
//   - python3 test_check.py        (governance-integrity checker)
//   - python3 test_yaml2json.py    (the YAML->JSON shim)
//   - node --test 'harness/test_*.mjs'      picks up test_gate + test_acceptance
//     + test_scorecard (top-level harness suites).
//   - node --test 'harness/arch/test_*.mjs' picks up test_arch-check (the 17
//     negative-fixture tests pinning every architecture check, incl. the
//     multi-line-string function-length regression). The non-recursive glob
//     above does NOT descend into harness/arch/, so without this second entry
//     those 17 tests would never run under `forge accept`.
// Both Node runs use a QUOTED glob (Node 26's `--test <dir>` is broken; letting
// Node's own runner resolve the glob dodges that directory bug) and carry
// FORGE_ACCEPT_INNER=1 so test_acceptance.mjs SKIPS its integration test that
// re-spawns the full acceptance.mjs — without the flag this probe would trigger
// ~4x redundant nested gate runs (acceptance -> glob -> test_acceptance -> ...).
// The arch run is also fail-CLOSED on discovery: exit 0 is necessary but not
// sufficient — a glob matching nothing still exits 0, so we require the runner's
// own `# tests N` summary with N > 0 (else the 17 tests could vanish unnoticed).
export function probeTests() {
  const archRun = run(
    'node',
    ['--test', '--test-reporter=tap', 'harness/arch/test_*.mjs'],
    { FORGE_ACCEPT_INNER: '1' },
  );
  const archCount = (archRun.out.match(/(?:^|\n)# tests (\d+)/) ?? [])[1];
  const archOk = archRun.ok && Number(archCount) > 0;
  const suites = [
    ['test_check.py', run('python3', [join(HARNESS_DIR, 'test_check.py')]).ok],
    ['test_yaml2json.py', run('python3', [join(HARNESS_DIR, 'test_yaml2json.py')]).ok],
    ['harness/test_*.mjs', run('node', ['--test', 'harness/test_*.mjs'], { FORGE_ACCEPT_INNER: '1' }).ok],
    ['harness/arch/test_*.mjs', archOk],
  ];
  const failed = suites.filter(([, ok]) => !ok).map(([name]) => name);
  const ok = failed.length === 0;
  const detail = ok
    ? `test_check.py + test_yaml2json.py + harness/test_*.mjs + harness/arch/test_*.mjs (${archCount}): all green`
    : `failed: ${failed.join(', ')}`;
  return result('test_pass', ok ? PASS : FAIL, detail);
}

// app_test_pass == true  <-  EVERY discovered example app's test suite is green.
// Without this probe, `forge accept` would happily ACCEPT a regressed app: no
// automated gate ran the app's tests. P21: discover all examples/<app>/ dirs
// that ship a test/ subdir (not just the hardcoded url-shortener), infer the
// runner per app from its test-file extensions, and FAIL if ANY app fails.

// Discover example apps: examples/<app>/ dirs that contain a test/ subdir.
// Returns [{name, testDir}] sorted by name (deterministic ordering).
function discoverApps() {
  const examples = join(ROOT, 'examples');
  if (!existsSync(examples)) return [];
  const apps = [];
  for (const name of readdirSync(examples).sort()) {
    const testDir = join(examples, name, 'test');
    let st;
    try { st = statSync(testDir); } catch { continue; }
    if (st.isDirectory()) apps.push({ name, testDir });
  }
  return apps;
}

// Infer a runner from the test files present. Order matters: a single app dir
// is assumed homogeneous; first matching language wins. null -> unknown layout.
function inferRunner(app) {
  const files = readdirSync(app.testDir);
  if (files.some((f) => f.endsWith('.test.mjs'))) return 'node';
  if (files.some((f) => f.endsWith('_test.py') || (f.startsWith('test_') && f.endsWith('.py')))) return 'python';
  if (files.some((f) => f.endsWith('_test.go'))) return 'go';
  return null;
}

// Run one app's suite; return {name, ok, detail}. Node apps preserve the Node-26
// quoted-glob workaround + fail-closed `tests N > 0` count; python/go shell their
// native discovery runners and judge on exit code.
function runApp(app) {
  const runner = inferRunner(app);
  if (runner === 'node') return runNodeApp(app);
  if (runner === 'python') {
    const r = run('python3', ['-m', 'unittest', 'discover', '-s', `examples/${app.name}/test`]);
    return { name: app.name, ok: r.ok, detail: r.ok ? `${app.name}: PASS (python)` : `${app.name}: FAIL (python exit ${r.code})` };
  }
  if (runner === 'go') {
    // -C runs from the app's own module dir, so a self-contained nested module
    // (its own go.mod, no root go.work) tests correctly — `go test ./examples/..`
    // from the repo root fails because the root is not a Go module.
    const r = run('go', ['-C', `examples/${app.name}`, 'test', './...']);
    return { name: app.name, ok: r.ok, detail: r.ok ? `${app.name}: PASS (go)` : `${app.name}: FAIL (go exit ${r.code})` };
  }
  return { name: app.name, ok: false, detail: `${app.name}: FAIL (no recognized test runner)` };
}

// Run a node app's *.test.mjs suite. Fail-closed: exit 0 is necessary but NOT
// sufficient — a glob matching nothing still exits 0 ("tests 0"). Require the
// runner's own `tests N` summary with N > 0. The quoted glob dodges Node 26's
// broken `--test <dir>`; the TAP reporter pins the summary format.
function runNodeApp(app) {
  const glob = `examples/${app.name}/test/*.test.mjs`;
  const r = run('node', ['--test', '--test-reporter=tap', glob]);
  const m = r.out.match(/(?:^|\n)# tests (\d+)/);
  const count = m ? Number(m[1]) : null;
  const ok = r.ok && count !== null && count > 0;
  const detail = ok
    ? `${app.name}: PASS (node, ${count} tests)`
    : count === 0 || count === null
      ? `${app.name}: FAIL (no tests discovered, expected >=1)`
      : `${app.name}: FAIL (node exit ${r.code})`;
  return { name: app.name, ok, detail };
}

export function probeAppTests() {
  const apps = discoverApps();
  if (apps.length === 0) {
    // Never a silent pass: zero apps is reported N/A with a reason.
    return result('app_test_pass', NA, 'no example apps with a test/ dir discovered under examples/');
  }
  const runs = apps.map(runApp);
  const failed = runs.filter((r) => !r.ok);
  const ok = failed.length === 0;
  const detail = runs.map((r) => r.detail).join('; ');
  return result('app_test_pass', ok ? PASS : FAIL, detail);
}

// Criteria with NO executable check in this repo. Surfaced as N/A with an
// honest reason; NEVER asserted as a pass (see decide()).
// NOTE: security_findings is NO LONGER here — harness/secret-scan.mjs gives it a
// real (hardcoded-secret) check, so probeSecurity reports PASS/FAIL, not N/A.
// The remaining four stay honestly N/A (no coverage/lint/typecheck/build tool).
export function probeNotApplicable() {
  return [
    result('coverage', NA, 'no coverage tool wired in this repo'),
    result('lint', NA, 'no linter configured (no eslint/ruff)'),
    result('typecheck', NA, 'no TS sources / type-checker in this repo'),
    result('build', NA, 'no build step (declarative + zero-dep harness)'),
  ];
}

// security_findings == 0  <-  hardcoded-secret scan (harness/secret-scan.mjs).
// Direction-5 security/compliance gate, aligned with OWASP Agentic Top-10 2025-12
// (sensitive information disclosure). PASS when the secret scanner exits 0 (no
// committed AWS keys / private-key headers / GitHub-Slack tokens / high-entropy
// credential assignments), FAIL otherwise. HONESTY: this is a PATTERN scanner
// for hardcoded secrets only — it is NOT a dependency/CVE (SCA) scanner; that
// needs a vulnerability DB and is the v3 roadmap item, deliberately out of scope.
// It is load-bearing (see LOAD_BEARING): a security gate that found a leaked
// credential yet still ACCEPTED would defeat its own purpose.
export function probeSecurity() {
  const r = run('node', [join(HARNESS_DIR, 'secret-scan.mjs')]);
  return result(
    'security_findings',
    r.ok ? PASS : FAIL,
    r.ok ? 'secret-scan: no hardcoded secrets (pattern scan; SCA/CVE out of scope)' : `secret-scan exit ${r.code}: ${r.out}`,
  );
}

// architecture == clean  <-  clean-architecture DEPENDENCY DIRECTION + size
// budgets (harness/arch/arch-check.mjs, all 8 checks: layering / package /
// fan-in / cognitive / anti-pattern-naming / function-length /
// circular-dependency / drift-guard). Distinct from arch_violations (check.py
// governance integrity): this FAILs when e.g. a domain package imports
// infrastructure (an inner layer imports an outer) or a function exceeds budget.
export function probeArchitecture() {
  const r = run('node', [join(HARNESS_DIR, 'arch', 'arch-check.mjs')]);
  return result(
    'architecture',
    r.ok ? PASS : FAIL,
    r.ok
      ? 'arch-check: layering/package/fan-in/cognitive/anti-pattern-naming/function-length/circular-dependency/drift-guard clean'
      : `arch-check exit ${r.code}`,
  );
}

// --- aggregation + verdict ---------------------------------------------------

// Gather every criterion result. Order mirrors the acceptance schema.
export function collect() {
  return [
    probeTests(),
    probeAppTests(),
    probeComplexity(),
    probeArch(),
    probeArchitecture(),
    probeSecurity(),
    ...probeNotApplicable(),
  ];
}

// The executable, load-bearing criteria: each MUST be present AND status===PASS
// for acceptance. Unlike the N/A criteria, these are backed by a real check, so
// "not FAIL" is not enough — a missing or non-PASS load-bearing criterion is a
// hard reject (e.g. a probe silently dropped, or surfaced as N/A by mistake).
export const LOAD_BEARING = ['test_pass', 'app_test_pass', 'complexity_violations', 'arch_violations', 'architecture', 'security_findings'];

// PURE verdict function (no I/O) so it is directly unit-testable.
//
// Hardened (P10): ACCEPTED requires ALL of —
//   (1) every status is a known verdict {PASS,FAIL,N-A} (unknown -> hard reject);
//   (2) the four load-bearing criteria are each PRESENT and PASS (not merely
//       not-FAIL); a missing/NA/unknown load-bearing criterion blocks accept;
//   (3) the results are non-empty (zero criteria prove nothing).
// N/A is explicitly NOT a pass for the remaining criteria: it neither blocks nor
// counts toward satisfaction — surfaced so missing coverage stays honest.
export function decide(results) {
  const failed = results.filter((r) => r.status === FAIL);
  const passed = results.filter((r) => r.status === PASS);
  const na = results.filter((r) => r.status === NA);
  const unknown = results.filter((r) => r.status !== PASS && r.status !== FAIL && r.status !== NA);
  const byName = new Map(results.map((r) => [r.criterion, r.status]));
  const missing = LOAD_BEARING.filter((name) => byName.get(name) !== PASS);
  const reasons = [
    ...failed.map((r) => `${r.criterion} failed`),
    ...unknown.map((r) => `${r.criterion} bad status ${JSON.stringify(r.status)}`),
    ...missing
      .filter((name) => byName.get(name) === undefined || byName.get(name) === NA)
      .map((name) => `${name} not satisfied (${byName.get(name) ?? 'absent'})`),
  ];
  const empty = results.length === 0;
  const accepted = reasons.length === 0 && !empty;
  return {
    accepted,
    failed: failed.length,
    passed: passed.length,
    na: na.length,
    line: accepted
      ? 'forge-accept: ACCEPTED'
      : `forge-accept: REJECTED — ${empty ? 'no criteria evaluated' : reasons.join('; ')}`,
  };
}

const ICON = { [PASS]: 'PASS', [FAIL]: 'FAIL', [NA]: 'N-A ' };

export function render(results, verdict) {
  const lines = results.map((r) => `  [${ICON[r.status]}] ${r.criterion} — ${r.detail}`);
  return [
    'forge-accept: acceptance gate (.agent/eval/acceptance.schema.yml)',
    ...lines,
    `  (${verdict.passed} pass · ${verdict.failed} fail · ${verdict.na} n/a — n/a is NOT counted as satisfied)`,
    verdict.line,
  ].join('\n');
}

// emitJson prints the per-criterion results as a JSON array
// [{criterion,status,detail}] to stdout — the structured mode consumed by
// forge-core's gate.ProbeAll. Reuses the same pure collect(); the exit code
// still reflects the verdict so `--json` stays usable as a gate too.
function emitJson() {
  const results = collect();
  console.log(JSON.stringify(results));
  process.exit(decide(results).accepted ? 0 : 1);
}

function main() {
  if (process.argv.slice(2).includes('--json')) {
    emitJson();
    return;
  }
  const results = collect();
  const verdict = decide(results);
  console.log(render(results, verdict));
  process.exit(verdict.accepted ? 0 : 1);
}

// Run only when executed directly, not on import (keeps the module test-safe).
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
