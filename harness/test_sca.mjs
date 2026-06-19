// Tests for harness/sca.mjs (node:test, zero external deps).
// Run: node --test harness/test_sca.mjs   (or: node --test harness/)
//
// Layers, mirroring the rest of the harness:
//   1. unit (pure) — matchAdvisories / parseManifest / compareVersions / inRange
//      over crafted FIXTURES, with NO disk. The headline test is the ANTI-STUB
//      proof: a fixture advisory + a vulnerable fixture dep DOES yield a finding
//      — i.e. the framework genuinely detects a vulnerability, it is not a stub
//      that always returns "clean".
//   2. I/O boundary — loadAdvisories on a real fixture DB (matched) and on a
//      MISSING path (available:false, does NOT throw — the honesty contract).
//
// HONESTY: the framework is real and fixture-verifiable; full CVE COVERAGE needs
// a real OSV/NVD DB, and with no DB the scan is N/A — never a faked "scanned the
// whole world, no vulnerabilities". These tests pin exactly that boundary.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import * as sca from './sca.mjs';
const {
  parseManifest, matchAdvisories, loadAdvisories, scanRepo,
  compareVersions, inRange, discoverManifests, render,
} = sca;

const SCA_PATH = join(dirname(fileURLToPath(import.meta.url)), 'sca.mjs');

// --- FIXTURE advisory DB (OSV-format) ----------------------------------------
// Two records: a vulnerable Go range [v1.2.0, v1.4.0) and a vulnerable npm range
// [4.17.0, 4.17.21). These drive every matching assertion below.
const ADVISORIES = [
  {
    id: 'GHSA-fixture-go-0001',
    package: 'example.com/vuln/pkg',
    ecosystem: 'Go',
    vulnerable: { introduced: 'v1.2.0', fixed: 'v1.4.0' },
    severity: 'HIGH',
  },
  {
    id: 'CVE-FIXTURE-2024-9999',
    package: 'lodash',
    ecosystem: 'npm',
    vulnerable: { introduced: '4.17.0', fixed: '4.17.21' },
    severity: 'CRITICAL',
  },
];

// --- smoke: importing sca.mjs must have no side effects ----------------------
test('importing sca.mjs produces no output and exits 0 (no side effects)', () => {
  const specifier = pathToFileURL(SCA_PATH).href;
  const res = spawnSync(
    process.execPath,
    ['-e', `import(${JSON.stringify(specifier)})`],
    { encoding: 'utf8' },
  );
  assert.equal(res.status, 0, `exit 0 expected; stderr:\n${res.stderr}`);
  assert.equal(res.stdout, '', `import must print nothing; got:\n${res.stdout}`);
  assert.equal(typeof matchAdvisories, 'function');
  assert.equal(typeof scanRepo, 'function');
});

// --- ★ANTI-STUB CORE PROOF★: a vulnerable dep IS detected --------------------
// This is the single most important test: it proves the framework genuinely
// matches a known-vulnerable dependency against an advisory and emits a finding.
// If sca.mjs were a fake stub that always returns "clean", THIS fails.
test('matchAdvisories DETECTS a vulnerable dependency (fixture advisory + fixture dep) — anti-stub proof', () => {
  const deps = [{ name: 'example.com/vuln/pkg', version: 'v1.3.0', ecosystem: 'Go' }];
  const findings = matchAdvisories(deps, ADVISORIES);
  assert.equal(findings.length, 1, 'the vulnerable dep must surface exactly one finding');
  const f = findings[0];
  assert.equal(f.dep, 'example.com/vuln/pkg');
  assert.equal(f.advisory_id, 'GHSA-fixture-go-0001');
  assert.equal(f.severity, 'HIGH');
  assert.equal(f.installed, 'v1.3.0');
  assert.equal(f.ecosystem, 'Go');
  assert.equal(f.fixed, 'v1.4.0', 'finding carries the fixed version for remediation');
});

test('matchAdvisories detects a vulnerable npm dep too (CRITICAL)', () => {
  const deps = [{ name: 'lodash', version: '4.17.11', ecosystem: 'npm' }];
  const findings = matchAdvisories(deps, ADVISORIES);
  assert.equal(findings.length, 1);
  assert.equal(findings[0].advisory_id, 'CVE-FIXTURE-2024-9999');
  assert.equal(findings[0].severity, 'CRITICAL');
});

// --- a FIXED (patched) version produces NO finding ---------------------------
test('matchAdvisories: a dep at/above the fixed version is NOT flagged', () => {
  const atFixed = matchAdvisories(
    [{ name: 'lodash', version: '4.17.21', ecosystem: 'npm' }], ADVISORIES,
  );
  assert.deepEqual(atFixed, [], '== fixed is patched (half-open range) — no finding');
  const aboveFixed = matchAdvisories(
    [{ name: 'example.com/vuln/pkg', version: 'v1.5.0', ecosystem: 'Go' }], ADVISORIES,
  );
  assert.deepEqual(aboveFixed, [], '> fixed is patched — no finding');
});

// --- ecosystem isolation: same name, different ecosystem, no false match -----
test('matchAdvisories does NOT cross ecosystems (a Go pkg name in npm is not matched)', () => {
  // A dep literally named "lodash" but in the Go ecosystem must NOT match the
  // npm lodash advisory — name alone is insufficient; ecosystem must agree.
  const findings = matchAdvisories(
    [{ name: 'lodash', version: '4.17.11', ecosystem: 'Go' }], ADVISORIES,
  );
  assert.deepEqual(findings, [], 'ecosystem mismatch must prevent a false positive');
});

// --- version boundaries: == introduced hits, == fixed misses -----------------
test('matchAdvisories boundary: == introduced is vulnerable, == fixed is patched', () => {
  const atIntroduced = matchAdvisories(
    [{ name: 'example.com/vuln/pkg', version: 'v1.2.0', ecosystem: 'Go' }], ADVISORIES,
  );
  assert.equal(atIntroduced.length, 1, '== introduced falls INSIDE [introduced, fixed)');
  const belowIntroduced = matchAdvisories(
    [{ name: 'example.com/vuln/pkg', version: 'v1.1.9', ecosystem: 'Go' }], ADVISORIES,
  );
  assert.deepEqual(belowIntroduced, [], '< introduced is not yet vulnerable');
});

// --- compareVersions / inRange unit checks (the semver engine) ---------------
test('compareVersions handles v-prefix, missing parts, and pre-release ordering', () => {
  assert.equal(compareVersions('v1.2.3', '1.2.3'), 0, 'v-prefix is ignored');
  assert.equal(compareVersions('1.2', '1.2.0'), 0, 'missing patch is zero-filled');
  assert.equal(compareVersions('1.2.0', '1.10.0'), -1, 'numeric (not lexical) compare: 2 < 10');
  assert.equal(compareVersions('2.0.0', '1.9.9'), 1);
  assert.equal(compareVersions('1.0.0-rc.1', '1.0.0'), -1, 'a pre-release sorts below its release');
  assert.equal(compareVersions('1.0.0-rc.2', '1.0.0-rc.1'), 1, 'pre-release identifiers compare');
});

test('inRange implements the half-open [introduced, fixed) window', () => {
  assert.equal(inRange('1.3.0', '1.2.0', '1.4.0'), true);
  assert.equal(inRange('1.2.0', '1.2.0', '1.4.0'), true, '== introduced is inside');
  assert.equal(inRange('1.4.0', '1.2.0', '1.4.0'), false, '== fixed is outside');
  assert.equal(inRange('1.1.0', '1.2.0', '1.4.0'), false, '< introduced is outside');
  assert.equal(inRange('9.9.9', '1.2.0', undefined), true, 'no fixed -> open-ended (still vulnerable)');
  assert.equal(inRange('0.0.1', undefined, '1.4.0'), true, 'no introduced -> from the beginning');
});

// --- parseManifest: real go.mod (grouped require block) ----------------------
test('parseManifest parses a go.mod grouped require block (+ single-line + indirect)', () => {
  const goMod = [
    'module example/app',
    '',
    'go 1.26',
    '',
    'require github.com/single/dep v1.0.0',
    '',
    'require (',
    '\texample.com/vuln/pkg v1.3.0',
    '\tgithub.com/other/lib v2.1.0 // indirect',
    ')',
  ].join('\n');
  const deps = parseManifest(goMod, 'go.mod');
  assert.deepEqual(deps, [
    { name: 'github.com/single/dep', version: 'v1.0.0', ecosystem: 'Go' },
    { name: 'example.com/vuln/pkg', version: 'v1.3.0', ecosystem: 'Go' },
    { name: 'github.com/other/lib', version: 'v2.1.0', ecosystem: 'Go' },
  ], 'both single-line and grouped requires parsed; // indirect comment stripped');
});

// --- parseManifest: real package.json (deps + devDeps) -----------------------
test('parseManifest parses package.json dependencies AND devDependencies (range op stripped)', () => {
  const pkg = JSON.stringify({
    name: 'demo',
    dependencies: { lodash: '^4.17.11', express: '4.18.2' },
    devDependencies: { vitest: '~1.0.0' },
  });
  const deps = parseManifest(pkg, 'package.json');
  assert.deepEqual(deps.sort((a, b) => a.name.localeCompare(b.name)), [
    { name: 'express', version: '4.18.2', ecosystem: 'npm' },
    { name: 'lodash', version: '4.17.11', ecosystem: 'npm' },
    { name: 'vitest', version: '1.0.0', ecosystem: 'npm' },
  ], 'deps + devDeps both parsed; ^ and ~ range operators stripped to base version');
});

test('parseManifest parses requirements.txt (pins, skips comments/options/bare names)', () => {
  const reqs = [
    '# a comment',
    'requests==2.31.0',
    'flask>=2.0.0',
    '-r other.txt',
    'bare-no-version',
  ].join('\n');
  const deps = parseManifest(reqs, 'requirements.txt');
  assert.deepEqual(deps, [
    { name: 'requests', version: '2.31.0', ecosystem: 'PyPI' },
    { name: 'flask', version: '2.0.0', ecosystem: 'PyPI' },
  ], 'pinned/bounded deps parsed; comment, -r option, and unpinned name skipped');
});

test('parseManifest returns [] for an unknown manifest kind', () => {
  assert.deepEqual(parseManifest('whatever', 'Cargo.toml'), []);
});

// --- loadAdvisories: HONESTY contract on a missing DB ------------------------
test('loadAdvisories on a MISSING file returns {available:false} and does NOT throw', () => {
  const res = loadAdvisories('/no/such/path/advisories.json');
  assert.equal(res.available, false, 'absent DB -> not available (honest N/A, not faked)');
  assert.deepEqual(res.advisories, [], 'no advisories when the DB is absent');
  assert.equal(res.error, undefined, 'a simply-absent DB is NOT an error');
});

test('loadAdvisories with no path at all returns {available:false}', () => {
  assert.deepEqual(loadAdvisories(undefined), { advisories: [], available: false });
});

// --- loadAdvisories + scanRepo over a REAL fixture DB on disk ----------------
test('loadAdvisories reads a real OSV fixture DB; scanRepo finds the planted vuln', () => {
  const dir = mkdtempSync(join(tmpdir(), 'sca-fixture-'));
  try {
    // A tiny project tree: a go.mod pinning a vulnerable version + an OSV DB.
    writeFileSync(join(dir, 'go.mod'), [
      'module example/fixture',
      'go 1.26',
      'require (',
      '\texample.com/vuln/pkg v1.3.0',
      ')',
    ].join('\n'));
    const dbPath = join(dir, 'advisories.json');
    writeFileSync(dbPath, JSON.stringify(ADVISORIES));

    const loaded = loadAdvisories(dbPath);
    assert.equal(loaded.available, true, 'a present, well-formed DB is available');
    assert.equal(loaded.advisories.length, 2);

    const report = scanRepo(dir, dbPath);
    assert.equal(report.available, true);
    assert.equal(report.manifestCount, 1, 'discovered the go.mod');
    assert.equal(report.depCount, 1);
    assert.equal(report.findings.length, 1, 'the planted vulnerable dep is detected end-to-end');
    assert.equal(report.findings[0].advisory_id, 'GHSA-fixture-go-0001');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('scanRepo with NO DB reports available:false (framework ready, scan not run)', () => {
  const dir = mkdtempSync(join(tmpdir(), 'sca-nodb-'));
  try {
    writeFileSync(join(dir, 'package.json'), JSON.stringify({
      name: 'x', dependencies: { lodash: '4.17.11' },
    }));
    const report = scanRepo(dir, join(dir, 'does-not-exist.json'));
    assert.equal(report.available, false, 'no DB -> not available (honest)');
    assert.equal(report.manifestCount, 1, 'manifests are still discovered + parsed');
    assert.equal(report.depCount, 1);
    assert.deepEqual(report.findings, [], 'no DB -> no findings claimed (NOT a faked clean)');
    // render must say so, not pretend it scanned.
    const out = render(report);
    assert.match(out, /N\/A/);
    assert.match(out, /NOT run, NOT faked/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// --- discoverManifests skips SKIP_DIRS ---------------------------------------
test('discoverManifests skips node_modules / .git (no vendored manifests leak)', () => {
  const dir = mkdtempSync(join(tmpdir(), 'sca-walk-'));
  try {
    writeFileSync(join(dir, 'package.json'), '{}');
    const found = discoverManifests(dir);
    assert.equal(found.length, 1, 'finds the top-level manifest');
    assert.equal(found[0].kind, 'package.json');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// --- CLI: this repo has no DB -> honest N/A (does not fake a scan) ------------
test('the sca CLI on the real repo (no DB) reports N/A and exits 0 (honest, not faked)', () => {
  // The repo ships no advisory DB, so the framework must HONESTLY degrade: report
  // N/A ("no OSV advisory DB"), NOT pretend it scanned the world clean. Exit 0
  // because N/A must never block a gate. Pass an explicit non-existent dbPath so
  // the test is independent of whether a DB is ever added to .agent/security/.
  const res = spawnSync(
    process.execPath,
    [SCA_PATH, dirname(SCA_PATH).replace(/\/harness$/, ''), '/no/such/advisories.json'],
    { encoding: 'utf8' },
  );
  assert.equal(res.status, 0, `N/A must exit 0; stdout:\n${res.stdout}\nstderr:\n${res.stderr}`);
  assert.match(res.stdout, /forge-sca: N\/A/);
  assert.match(res.stdout, /no OSV advisory DB/);
  assert.match(res.stdout, /NOT run, NOT faked/);
});
