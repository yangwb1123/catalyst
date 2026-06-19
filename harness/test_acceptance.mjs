// Tests for harness/acceptance.mjs (node:test, zero external deps).
// Run: node --test harness/test_acceptance.mjs   (or: node --test harness/)
//
// Two layers, mirroring the gate's design:
//   1. integration — drive the real CLI against the real repo (ACCEPTED, exit 0).
//   2. unit — exercise the PURE `decide()` verdict over crafted result arrays,
//      including the honesty invariant that N/A is NOT counted as satisfied.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import * as acc from './acceptance.mjs';
const { decide, PASS, FAIL, NA, LOAD_BEARING, probeNotApplicable, probeCoverage } = acc;

// allPass builds a results array where every load-bearing criterion is PASS,
// then applies the given overrides — so a test can isolate ONE criterion's
// status without tripping the P10 "all four load-bearing must PASS" guard.
const allPass = (overrides = []) => {
  const base = LOAD_BEARING.map((criterion) => ({ criterion, status: PASS, detail: 'x' }));
  for (const o of overrides) {
    const hit = base.find((r) => r.criterion === o.criterion);
    if (hit) hit.status = o.status;
    else base.push({ criterion: o.criterion, status: o.status, detail: 'x' });
  }
  return base;
};

const ACCEPT_PATH = join(dirname(fileURLToPath(import.meta.url)), 'acceptance.mjs');

// Helper: build a result row the way the gate's probes do.
const row = (criterion, status) => ({ criterion, status, detail: 'x' });

// --- smoke: importing acceptance.mjs must have no side effects (main not run) -
test('importing acceptance.mjs produces no output and exits 0 (no side effects)', () => {
  const specifier = pathToFileURL(ACCEPT_PATH).href;
  const res = spawnSync(
    process.execPath,
    ['-e', `import(${JSON.stringify(specifier)})`],
    { encoding: 'utf8' },
  );
  assert.equal(res.status, 0, `exit 0 expected; stderr:\n${res.stderr}`);
  assert.equal(res.stdout, '', `import must print nothing; got:\n${res.stdout}`);
  assert.equal(typeof decide, 'function');
});

// --- integration: real repo must be ACCEPTED with exit 0 ---------------------
// SKIPPED when FORGE_ACCEPT_INNER is set: that env flag means this suite is
// itself running *inside* acceptance.mjs's probeTests() glob run. Re-spawning
// the whole acceptance gate from there would recurse (acceptance -> glob ->
// test_acceptance -> acceptance -> ...) and cost ~4x redundant nested gate runs.
// The outer `node harness/acceptance.mjs` invocation (no flag) still runs it.
test('acceptance gate ACCEPTS the real repo and exits 0', { skip: Boolean(process.env.FORGE_ACCEPT_INNER) }, () => {
  const res = spawnSync(process.execPath, [ACCEPT_PATH], { encoding: 'utf8' });
  assert.equal(res.status, 0, `expected exit 0; got ${res.status}\n${res.stdout}\n${res.stderr}`);
  assert.match(res.stdout, /forge-accept: ACCEPTED/);
  // The four real, executable criteria must show PASS.
  assert.match(res.stdout, /\[PASS\] test_pass/);
  // app_test_pass proves the dogfood url-shortener suite is actually gated here
  // (a regression in the app would FAIL this criterion and REJECT the repo).
  assert.match(res.stdout, /\[PASS\] app_test_pass/);
  // DECLARATION-DRIVEN routing (gap closed): go-taskd runs via the ADAPTER `test:`
  // command (`go test ./...`), while url-shortener honestly FALLS BACK because the
  // typescript adapter's `vitest run` does not fit its node:test *.test.mjs — and
  // the fail-closed 47-tests count is preserved on that fallback path.
  assert.match(res.stdout, /go-taskd: PASS \(adapter: go test \.\/\.\.\.\)/);
  assert.match(res.stdout, /url-shortener: PASS \(node fallback: .*vitest.* does not fit app layout, 47 tests\)/);
  assert.match(res.stdout, /\[PASS\] complexity_violations/);
  assert.match(res.stdout, /\[PASS\] arch_violations/);
  // security_findings is now a REAL check (harness/secret-scan.mjs), not N/A:
  // the repo ships no hardcoded secret, so it must show PASS.
  assert.match(res.stdout, /\[PASS\] security_findings/);
  // N/A criteria must remain visible (honesty: never silently dropped). coverage
  // is now framework-backed (probeCoverage shells the adapter coverage tools);
  // it stays N/A here because no coverage tool is runnable+configured (go is
  // installed but the repo root is not a Go module; pytest/vitest absent), so it
  // remains NON-load-bearing and the repo is still ACCEPTED.
  assert.match(res.stdout, /\[N-A \] coverage/);
  assert.match(res.stdout, /\[N-A \] build/);
  // Honesty regression guard: go IS installed, so the coverage detail must not
  // dishonestly claim "go not installed" (the `go --version` exit-2 trap).
  assert.doesNotMatch(res.stdout, /go not installed/);
});

// --- coverage is now a real probe, not a hardcoded N/A in probeNotApplicable --
test('probeNotApplicable no longer carries coverage (it is a real probe now)', () => {
  const names = probeNotApplicable().map((r) => r.criterion);
  assert.ok(!names.includes('coverage'), 'coverage must NOT be a static N/A anymore');
  // typecheck/build remain the only genuinely-unwired criteria.
  assert.deepEqual(names.sort(), ['build', 'typecheck']);
});

test('probeCoverage yields a single, honest coverage row (the wired-in probe)', () => {
  // NOTE: deliberately tests probeCoverage() directly — NOT collect() — because
  // collect() runs probeTests()/probeAppTests(), which spawn `node --test`; doing
  // that from inside `node --test harness/test_*.mjs` would re-enter this suite
  // and recurse. probeCoverage shells only the (absent/unrunnable) coverage tools,
  // so it is cheap and side-effect-light, like the probeLint real-repo test.
  const r = probeCoverage();
  assert.equal(r.criterion, 'coverage', 'exactly the coverage criterion');
  assert.ok([PASS, FAIL, NA].includes(r.status), 'coverage status must be an honest verdict');
});

test('coverage is NOT load-bearing (an N/A coverage must not block accept)', () => {
  // Backward-compat invariant: coverage staying N/A keeps the repo ACCEPTED.
  assert.ok(!LOAD_BEARING.includes('coverage'), 'coverage must stay non-load-bearing');
  const base = LOAD_BEARING.map((criterion) => ({ criterion, status: PASS, detail: 'x' }));
  base.push({ criterion: 'coverage', status: NA, detail: 'no runnable coverage tool' });
  assert.equal(decide(base).accepted, true, 'N/A coverage must not block acceptance');
});

// --- unit: decide() is a pure verdict over a results array -------------------
test('decide() returns REJECTED when any criterion FAILs', () => {
  const v = decide(allPass([{ criterion: 'arch_violations', status: FAIL }, { criterion: 'coverage', status: NA }]));
  assert.equal(v.accepted, false);
  assert.equal(v.failed, 1);
  assert.match(v.line, /forge-accept: REJECTED — .*arch_violations failed/);
});

test('decide() returns ACCEPTED when load-bearing PASS and the rest are PASS or N-A', () => {
  const v = decide(allPass([{ criterion: 'lint', status: NA }, { criterion: 'build', status: NA }]));
  assert.equal(v.accepted, true);
  assert.equal(v.failed, 0);
  assert.equal(v.line, 'forge-accept: ACCEPTED');
});

test('decide() REJECTS when the app suite (app_test_pass) FAILs', () => {
  // Guards the structural fix: a regressed url-shortener app must block accept.
  const v = decide(allPass([{ criterion: 'app_test_pass', status: FAIL }]));
  assert.equal(v.accepted, false);
  assert.equal(v.failed, 1);
  assert.match(v.line, /forge-accept: REJECTED — .*app_test_pass failed/);
});

test('decide() names multiple failures in the REJECTED message', () => {
  const v = decide(allPass([
    { criterion: 'test_pass', status: FAIL },
    { criterion: 'arch_violations', status: FAIL },
  ]));
  assert.equal(v.accepted, false);
  assert.match(v.line, /test_pass failed/);
  assert.match(v.line, /arch_violations failed/);
});

// --- P10: load-bearing criteria must be PRESENT and PASS ---------------------
test('decide() REJECTS when a load-bearing criterion is missing entirely', () => {
  // Only three of the four executable criteria present (no app_test_pass).
  const results = [row('test_pass', PASS), row('complexity_violations', PASS), row('arch_violations', PASS)];
  const v = decide(results);
  assert.equal(v.accepted, false, 'a missing load-bearing criterion must block accept');
  assert.match(v.line, /app_test_pass not satisfied \(absent\)/);
});

test('decide() REJECTS when a load-bearing criterion is N-A (not merely not-FAIL)', () => {
  // complexity surfaced as N/A — "not FAIL" must NOT be enough to accept.
  const v = decide(allPass([{ criterion: 'complexity_violations', status: NA }]));
  assert.equal(v.accepted, false, 'an N/A load-bearing criterion must block accept');
  assert.match(v.line, /complexity_violations not satisfied \(N-A\)/);
});

test('decide() hard-rejects an unknown status value', () => {
  const v = decide(allPass([{ criterion: 'coverage', status: 'MAYBE' }]));
  assert.equal(v.accepted, false, 'an unrecognized status must be a hard reject');
  assert.match(v.line, /coverage bad status/);
});

test('decide() REJECTS empty results (zero criteria prove nothing)', () => {
  const v = decide([]);
  assert.equal(v.accepted, false);
  assert.match(v.line, /no criteria evaluated/);
});

// --- honesty invariant: N/A is NOT counted as a pass / as satisfied ----------
test('decide() does NOT count N-A as passed', () => {
  const results = [row('coverage', NA), row('lint', NA), row('build', NA)];
  const v = decide(results);
  // All N/A: zero passes (so N/A cannot masquerade as satisfaction).
  assert.equal(v.passed, 0, 'N/A must not inflate the pass count');
  assert.equal(v.na, 3);
  // It is not ACCEPTED here only because the load-bearing criteria are absent —
  // N/A itself never FAILs, but it also never satisfies a required criterion.
  assert.equal(v.failed, 0);
});

test('decide() keeps pass and n/a tallies separate', () => {
  const results = [row('test_pass', PASS), row('coverage', NA), row('lint', NA)];
  const v = decide(results);
  assert.equal(v.passed, 1, 'only the real PASS counts toward passed');
  assert.equal(v.na, 2, 'N/A tallied separately, not folded into passed');
});
