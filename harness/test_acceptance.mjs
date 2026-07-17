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
import { readFileSync, mkdtempSync, writeFileSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import * as acc from './acceptance.mjs';
import { resolveCoverageThreshold, judgeCoverage, computeCoverageThreshold } from './adapters.mjs';
import { parseRules } from './arch/scan.mjs';
import { categorize, withCategory, APPLICABLE, INAPPLICABLE, NO_TOOL, ROOT } from './acceptance-kernel.mjs';
const { decide, PASS, FAIL, NA, LOAD_BEARING, probeNotApplicable, probeCoverage, probeSCA, runCountedTest, runPythonSuites } = acc;

// allPass builds a results array where every load-bearing criterion is PASS,
// then applies the given overrides — so a test can isolate ONE criterion's
// status without tripping the P10 "all six load-bearing must PASS" guard.
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
// HOST-AGNOSTIC by design: this suite ships VERBATIM into every scaffolded
// project (it is in forge-init's COPIED_FILES), so it drives `node
// harness/acceptance.mjs` against WHATEVER repo it lands in and asserts only the
// invariants that hold for ANY ForgeOS project — the ACCEPTED verdict, the
// load-bearing criteria PASSing, N/A staying honestly visible, and the
// declaration-driven app-test routing being live. It must NOT hardcode the
// SOURCE repo's app names (go-taskd / url-shortener) or environment (go
// installed), or it would falsely fail when copied to a scaffold that ships only
// examples/starter/ — turning copy-anywhere into a lie patched over by the INNER
// skip. (The skip below still prevents recursion when this runs inside
// acceptance.mjs's own probeTests glob; it is not what makes the asserts portable.)
test('acceptance gate ACCEPTS the repo it runs in and exits 0', { skip: Boolean(process.env.FORGE_ACCEPT_INNER) }, () => {
  const res = spawnSync(process.execPath, [ACCEPT_PATH], { encoding: 'utf8' });
  assert.equal(res.status, 0, `expected exit 0; got ${res.status}\n${res.stdout}\n${res.stderr}`);
  assert.match(res.stdout, /forge-accept: ACCEPTED/);
  // The six load-bearing, executable criteria must each show PASS (asserted below).
  assert.match(res.stdout, /\[PASS\] test_pass/);
  // app_test_pass proves the discovered example app suite is actually gated here
  // (a regression in any app would FAIL this criterion and REJECT the repo).
  assert.match(res.stdout, /\[PASS\] app_test_pass/);
  // DECLARATION-DRIVEN routing must be LIVE, asserted host-agnostically: the
  // app_test_pass detail must carry an `adapter:` tag (an app whose adapter
  // `test:` command fits its layout — e.g. the source repo's go-taskd `go test
  // ./...`) OR a `fallback:` tag (an app that honestly falls back, e.g. a
  // node:test *.test.mjs app the typescript adapter's `vitest run` does not fit —
  // url-shortener in the source repo, starter in a fresh scaffold). Either tag
  // proves the appTestPlan routing ran; we do NOT bind to a specific app name or
  // test count, which differ per project.
  const appLine = (res.stdout.match(/\[PASS\] app_test_pass — (.*)/) ?? [])[1] ?? '';
  assert.match(appLine, /\b(?:adapter|fallback):/, `app_test_pass detail must show a live adapter/fallback test route; got: ${appLine}`);
  assert.match(res.stdout, /\[PASS\] complexity_violations/);
  assert.match(res.stdout, /\[PASS\] arch_violations/);
  // architecture (clean-architecture dependency-direction + size budgets) is
  // load-bearing and must PASS for any clean tree.
  assert.match(res.stdout, /\[PASS\] architecture/);
  // security_findings is a REAL check (harness/secret-scan.mjs), not N/A: a clean
  // repo ships no hardcoded secret, so it must show PASS.
  assert.match(res.stdout, /\[PASS\] security_findings/);
  // HONESTY (host-agnostic): the unwired criteria must remain VISIBLE as N/A —
  // never silently dropped, never faked into a pass. coverage is framework-backed
  // (probeCoverage shells the adapter coverage tools) but stays N/A wherever no
  // coverage tool is runnable+configured; build has no wired step. Both are
  // NON-load-bearing, so an honest N/A keeps the repo ACCEPTED. (We assert N/A
  // presence, not the per-language reason, which is environment-specific.)
  assert.match(res.stdout, /\[N-A \] coverage/);
  assert.match(res.stdout, /\[N-A \] build/);
  // HONESTY tally guard (host-agnostic): the footer must show N/A counted
  // SEPARATELY and explicitly NOT folded into satisfaction — the core invariant
  // that an N/A can never masquerade as a pass, true for any project.
  assert.match(res.stdout, /n\/a is NOT counted as satisfied/);
});

// --- fail-CLOSED test discovery: a zero-match glob must NOT report green ------
// Pins the gap closed in probeTests' harness/test_*.mjs entry: `node --test`
// exits 0 on a glob matching ZERO files ("# tests 0"), so the old `.ok`-only
// judgement reported the load-bearing self-test suite GREEN while running
// nothing (exactly what happened when forge-init dropped test_enforce.mjs).
// runCountedTest now backs BOTH Node globs; this proves the symmetric guard.
test('runCountedTest is fail-CLOSED: a zero-match glob is NOT ok (the plugged blind spot)', () => {
  // A glob that matches no file at all. Pre-fix, probeTests judged this entry by
  // the child run's `.ok` alone — and `node --test <no-match-glob>` EXITS 0, so
  // it was a false green. The counted runner must reject it (count 0 -> ok:false).
  const r = runCountedTest('harness/__no_such_suite_*.mjs', { FORGE_ACCEPT_INNER: '1' });
  assert.equal(r.ok, false, 'a zero-match glob must be fail-closed (would have been a false green pre-fix)');
  assert.equal(r.count, 0, 'node --test reports "# tests 0" for a zero-match glob');
  // Belt-and-suspenders: a glob that DOES match real suites stays green with N>0,
  // so the guard rejects only the empty case (no false negative on real suites).
  const real = runCountedTest('harness/arch/test_*.mjs', { FORGE_ACCEPT_INNER: '1' });
  assert.equal(real.ok, true, 'a glob matching real suites must still pass');
  assert.ok(real.count > 0, 'real suites report N>0 discovered tests');
});

// --- fail-CLOSED Python suite discovery (the .mjs/.py asymmetry, now fixed) ---
// probeTests once HARDCODED the two Python suites, so a newly ADDED harness/
// test_*.py was never executed — a red new Python suite shipped while test_pass
// stayed green. runPythonSuites now DISCOVERS them (readdir), fail-closed, mirroring
// the Node globs. These pin: a red discovered suite fails, and an empty / missing-
// required walk is itself a FAIL row (a broken walker can't vacuously pass).
test('runPythonSuites DISCOVERS + runs every test_*.py, fail-CLOSED (the .py blind spot)', () => {
  const dir = mkdtempSync(join(tmpdir(), 'pysuite-'));
  try {
    // Stage the two known suites green + a NEW red one (the previously-ignored case).
    writeFileSync(join(dir, 'test_check.py'), 'import sys; sys.exit(0)\n');
    writeFileSync(join(dir, 'test_yaml2json.py'), 'import sys; sys.exit(0)\n');
    writeFileSync(join(dir, 'test_zzz_new.py'), 'import sys; sys.exit(1)\n'); // a red ADDED suite
    const { entries, count } = runPythonSuites(dir);
    assert.equal(count, 3, 'all three test_*.py are discovered (pre-fix the new one was invisible)');
    const red = entries.find(([n]) => n === 'test_zzz_new.py');
    assert.deepEqual(red, ['test_zzz_new.py', false], 'the newly added red suite is RUN and fails (was never run pre-fix)');
    assert.ok(entries.some(([, ok]) => ok === false), 'overall NOT all-ok -> probeTests would REJECT');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('runPythonSuites is fail-CLOSED on an empty / missing-required walk (no vacuous pass)', () => {
  const dir = mkdtempSync(join(tmpdir(), 'pyempty-'));
  try {
    const empty = runPythonSuites(dir); // no test_*.py at all
    assert.ok(empty.entries.some(([n, ok]) => ok === false && n.includes('discovery')),
      'an empty walk emits a fail-closed discovery row (a broken walker cannot pass)');
    // A walk that finds suites but MISSES a required one is also fail-closed.
    writeFileSync(join(dir, 'test_only_one.py'), 'import sys; sys.exit(0)\n');
    const partial = runPythonSuites(dir);
    assert.ok(partial.entries.some(([n, ok]) => ok === false && n.includes('test_check.py')),
      'missing a required suite (test_check.py) is a fail-closed discovery row');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

// --- coverage is now a real probe, not a hardcoded N/A in probeNotApplicable --
test('probeNotApplicable no longer carries coverage (it is a real probe now)', () => {
  const names = probeNotApplicable().map((r) => r.criterion);
  assert.ok(!names.includes('coverage'), 'coverage must NOT be a static N/A anymore');
  // typecheck/build remain the only genuinely-unwired criteria.
  assert.deepEqual(names.sort(), ['build', 'typecheck']);
});

// --- lifecycle-aware N/A categorisation (the bridge to forge-core's exemption) --
// categorize() is the PURE classifier forge-core's lifecycle-aware matrix relies
// on: a real verdict is APPLICABLE; an N/A is INAPPLICABLE only when its detail
// denotes absence-of-concept, else NO_TOOL (the fail-safe default). These pin the
// exact mapping of every N/A detail string the probes actually emit.
test('categorize: a verdict (PASS/FAIL) is APPLICABLE', () => {
  assert.equal(categorize(PASS, 'anything'), APPLICABLE);
  assert.equal(categorize(FAIL, 'anything'), APPLICABLE);
});

test('categorize: absence-of-concept N/A details -> INAPPLICABLE', () => {
  for (const detail of [
    'no build step (declarative + zero-dep harness)',
    'no source languages detected',
    'go: adapter has no lint command',
    'go: adapter has no coverage command',
    'no TS sources / type-checker in this repo',
    'no example apps with a test/ dir discovered under examples/',
  ]) {
    assert.equal(categorize(NA, detail), INAPPLICABLE, `inapplicable: ${detail}`);
  }
});

test('categorize: missing/unconfigured-TOOL N/A details -> NO_TOOL', () => {
  for (const detail of [
    'go: golangci-lint not installed',
    'js: eslint installed but unconfigured (no project config) — not run',
    'go: go installed but could not run here (no module/tests/config) — not a coverage verdict',
    'go: go produced no parseable coverage % (exit 1) — not a verdict',
    'SCA framework ready; no OSV advisory DB — dependency CVE scan not run, not faked',
  ]) {
    assert.equal(categorize(NA, detail), NO_TOOL, `no_tool: ${detail}`);
  }
});

test('categorize: an unclassifiable N/A is NO_TOOL (fail-safe toward strict)', () => {
  // The conservative default: a detail that matches no inapplicable phrase is
  // treated as a fixable tooling gap, never a free production exemption.
  assert.equal(categorize(NA, 'some unforeseen reason'), NO_TOOL);
  assert.equal(categorize(NA, ''), NO_TOOL);
  assert.equal(categorize(NA, undefined), NO_TOOL);
});

test('withCategory: ADDITIVE — preserves the row and stamps a category', () => {
  const r = withCategory({ criterion: 'build', status: NA, detail: 'no build step here' });
  assert.equal(r.criterion, 'build');
  assert.equal(r.status, NA);
  assert.equal(r.detail, 'no build step here');
  assert.equal(r.category, INAPPLICABLE, 'build with no-build-step detail is inapplicable');
});

test('collect() stamps a category on every row (the --json bridge payload)', () => {
  // The integration `collect()` runs probes that spawn `node --test`; from inside
  // `node --test` that recurses. So assert the contract on a SYNTHETIC row array put
  // through the same withCategory map collect() applies — proving every row carries
  // an honest category (a verdict is APPLICABLE, an N/A is classified).
  const rows = [
    { criterion: 'test_pass', status: PASS, detail: 'green' },
    { criterion: 'lint', status: NA, detail: 'go: golangci-lint not installed' },
    { criterion: 'build', status: NA, detail: 'no build step (declarative + zero-dep harness)' },
  ].map(withCategory);
  assert.equal(rows[0].category, APPLICABLE);
  assert.equal(rows[1].category, NO_TOOL);
  assert.equal(rows[2].category, INAPPLICABLE);
  assert.ok(rows.every((r) => typeof r.category === 'string' && r.category), 'every row has a category');
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

// --- coverage threshold is the project's mode×lifecycle floor, not hardcoded 60 -
test('probeCoverage wires the project mode×lifecycle threshold (the central knob), but an N/A is unchanged by it', () => {
  // The gap closed: the floor flows from .agent/project.yml × modes.yml, NOT the old
  // hardcoded 60. host-AGNOSTIC (this test ships VERBATIM to scaffolded projects of
  // ANY mode×lifecycle): instead of hardcoding the host's number, read the host's OWN
  // project.yml × modes.yml and assert the on-disk resolve equals the pure computation
  // for that exact pair — proving the wire is live without assuming WHICH knob the host
  // is set to (engineering×mvp here -> 80; a balanced×mvp scaffold -> 60; both pass).
  const repoRoot = dirname(fileURLToPath(import.meta.url)).replace(/\/harness$/, '');
  const project = parseRules(readFileSync(join(repoRoot, '.agent', 'project.yml'), 'utf8'));
  const modes = parseRules(readFileSync(join(repoRoot, '.agent', 'policies', 'modes.yml'), 'utf8'));
  const expected = computeCoverageThreshold(modes, project.mode, project.lifecycle);
  assert.equal(resolveCoverageThreshold(repoRoot), expected, `${project.mode}×${project.lifecycle} resolves to its computed floor ${expected}`);
  // HONESTY/backward-compat: the threshold only moves the PASS/FAIL boundary when a
  // tool actually RUNS and emits a %. This repo has no runnable coverage tool, so
  // the criterion is N/A — and changing the floor cannot turn that N/A into a
  // verdict. Pin that: probeCoverage stays N/A regardless of the resolved threshold.
  assert.equal(probeCoverage().status, NA, 'no runnable tool -> N/A, unaffected by the resolved threshold');
});

test('judgeCoverage honors the resolved threshold at the PASS/FAIL boundary (60 vs 80)', () => {
  // Proof that the resolved floor is load-bearing WHEN a tool runs: a real 75%
  // coverage run PASSES at the balanced floor (60) but FAILS at the engineering
  // floor (80) — exactly the boundary mode×lifecycle is meant to move.
  const ran75 = { ok: true, code: 0, out: 'coverage: 75.0% of statements' };
  assert.equal(judgeCoverage('go', 'go', true, ran75, 60).status, PASS, '75% >= 60 (balanced) PASSES');
  assert.equal(judgeCoverage('go', 'go', true, ran75, 80).status, FAIL, '75% < 80 (engineering) FAILS');
});

test('coverage is NOT load-bearing (an N/A coverage must not block accept)', () => {
  // Backward-compat invariant: coverage staying N/A keeps the repo ACCEPTED.
  assert.ok(!LOAD_BEARING.includes('coverage'), 'coverage must stay non-load-bearing');
  const base = LOAD_BEARING.map((criterion) => ({ criterion, status: PASS, detail: 'x' }));
  base.push({ criterion: 'coverage', status: NA, detail: 'no runnable coverage tool' });
  assert.equal(decide(base).accepted, true, 'N/A coverage must not block acceptance');
});

// --- dependency_vulnerabilities (SCA) is a real probe, honest N/A, non-blocking -
test('probeSCA yields a single, honest dependency_vulnerabilities row', () => {
  // Like probeCoverage, exercise probeSCA() directly (NOT collect(), which spawns
  // node --test and would re-enter this suite). COPY-ANYWHERE: this test file is
  // itself one of COPIED_FILES, so it must not hardcode THIS repo's DB state —
  // a scaffolded project has no .agent/security/advisories.json (that snapshot is
  // repo-specific data, not part of the universal template) and must honestly get
  // N/A, while this source repo ships a REAL OSV snapshot (harness/sca_fetch.mjs,
  // run against the live OSV API for its one actual dependency, PyYAML — see that
  // file's header for why it's a static, operator-refreshed snapshot rather than a
  // live query on every gate run) and must honestly get PASS. Branch on whether the
  // conventional DB path actually exists on THIS host, exactly like probeSCA itself:
  // same precedence (FORGE_SCA_DB wins if SET, else the conventional path) AND an
  // actual existsSync check (not just "the env var happens to be set" — a
  // FORGE_SCA_DB pointing at a nonexistent file must still resolve to N/A, same
  // as probeSCA's own loadAdvisories does when the path is unreadable).
  const dbPath = process.env.FORGE_SCA_DB || join(ROOT, '.agent', 'security', 'advisories.json');
  const dbPresent = existsSync(dbPath);
  const r = probeSCA();
  assert.equal(r.criterion, 'dependency_vulnerabilities', 'exactly the SCA criterion');
  assert.ok([PASS, FAIL, NA].includes(r.status), 'SCA status must be an honest verdict');
  if (dbPresent) {
    assert.equal(r.status, PASS, 'a real advisory DB with 0 known-vulnerable deps must be PASS, not N/A');
    assert.match(r.detail, /0 known-vulnerable/i, 'PASS detail must say what was actually checked');
  } else {
    assert.equal(r.status, NA, 'no advisory DB on this host -> honest N/A');
    assert.match(r.detail, /not run, not faked/i, 'N/A reason must say it was NOT faked');
  }
});

test('dependency_vulnerabilities is NOT load-bearing (an N/A SCA must not block accept)', () => {
  // Backward-compat invariant (mirrors coverage): with no advisory DB the honest
  // N/A keeps the repo ACCEPTED — adding SCA must not regress a clean accept.
  assert.ok(!LOAD_BEARING.includes('dependency_vulnerabilities'), 'SCA must stay non-load-bearing');
  const base = LOAD_BEARING.map((criterion) => ({ criterion, status: PASS, detail: 'x' }));
  base.push({ criterion: 'dependency_vulnerabilities', status: NA, detail: 'no advisory DB' });
  assert.equal(decide(base).accepted, true, 'N/A SCA must not block acceptance');
});

test('decide() REJECTS when a SCA vuln is found (DB present + vulnerable dep -> FAIL)', () => {
  // When an advisory DB IS supplied and a dependency matches a vulnerable range,
  // probeSCA returns FAIL and decide()'s normal fail path must block the accept —
  // the honesty mirror of N/A: present+vulnerable BLOCKS, absent stays N/A.
  const base = LOAD_BEARING.map((criterion) => ({ criterion, status: PASS, detail: 'x' }));
  base.push({ criterion: 'dependency_vulnerabilities', status: FAIL, detail: 'sca: 1 vulnerable dep' });
  const v = decide(base);
  assert.equal(v.accepted, false, 'a SCA FAIL must block accept');
  assert.match(v.line, /dependency_vulnerabilities failed/);
});

test('probeSCA with a real OSV DB (via FORGE_SCA_DB) DETECTS a planted vuln -> FAIL', () => {
  // End-to-end through the acceptance probe: a temp project with a vulnerable
  // go.mod dep + an OSV advisory DB pointed at by FORGE_SCA_DB must yield a FAIL
  // whose detail names the advisory id + severity. This is the gate-level
  // anti-stub proof — the probe is wired to the real matching engine, not a stub.
  const dir = mkdtempSync(join(tmpdir(), 'sca-acc-'));
  const prev = process.env.FORGE_SCA_DB;
  try {
    writeFileSync(join(dir, 'go.mod'), 'module x\ngo 1.26\nrequire (\n\texample.com/vuln/pkg v1.3.0\n)\n');
    const dbPath = join(dir, 'osv.json');
    writeFileSync(dbPath, JSON.stringify([{
      id: 'GHSA-acc-0001', package: 'example.com/vuln/pkg', ecosystem: 'Go',
      vulnerable: { introduced: 'v1.2.0', fixed: 'v1.4.0' }, severity: 'HIGH',
    }]));
    process.env.FORGE_SCA_DB = dbPath;
    const r = probeSCA(dir);
    assert.equal(r.status, FAIL, 'a vulnerable dep with a DB present must FAIL');
    assert.match(r.detail, /GHSA-acc-0001/, 'detail names the advisory id');
    assert.match(r.detail, /HIGH/, 'detail names the severity');
  } finally {
    if (prev === undefined) delete process.env.FORGE_SCA_DB; else process.env.FORGE_SCA_DB = prev;
    rmSync(dir, { recursive: true, force: true });
  }
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
  // Only three of the six load-bearing criteria present (no app_test_pass).
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
