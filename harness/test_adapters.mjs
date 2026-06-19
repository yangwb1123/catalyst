// Tests for harness/adapters.mjs + the lint criterion it powers in
// harness/acceptance.mjs (node:test, zero external deps).
// Run: node --test harness/test_adapters.mjs
//
// Two layers, mirroring the design split:
//   1. adapters.mjs — loadAdapter / detectLanguages (I/O) + the pure helpers
//      (langForExt / lintBinary / adapterCommands).
//   2. acceptance.mjs lint criterion — the PURE judgeLint decision (the
//      honesty/fail-safe branches: missing linter -> N/A, unconfigured -> N/A,
//      clean -> PASS, real violations -> FAIL) and probeLint over the real repo.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';

import {
  loadAdapter,
  detectLanguages,
  langForExt,
  lintBinary,
  adapterCommands,
  ADAPTER_LANGS,
  coverageBinary,
  coverageUnrunnable,
  parseCoveragePercent,
  judgeCoverage,
  versionProbeArgs,
  coverageArtifact,
  DEFAULT_COVERAGE_THRESHOLD,
} from './adapters.mjs';
import { judgeLint, unconfigured, probeLint, probeCoverage, PASS, FAIL, NA } from './acceptance.mjs';

// --- loadAdapter: reads each shipped adapter's lint command ------------------

test('loadAdapter reads the go/python/typescript lint commands from the yml maps', () => {
  // The first token of each lint command is the linter binary the probe checks.
  assert.equal(loadAdapter('go').lint, 'golangci-lint run ./...');
  assert.equal(loadAdapter('python').lint, 'ruff check .');
  assert.equal(loadAdapter('typescript').lint, 'eslint . --max-warnings=0');
});

test('loadAdapter also surfaces test + coverage commands', () => {
  const go = loadAdapter('go');
  assert.equal(go.test, 'go test ./...');
  assert.equal(go.coverage, 'go test -coverprofile=coverage.out ./...');
  assert.equal(loadAdapter('python').test, 'pytest -q');
  assert.equal(loadAdapter('typescript').test, 'vitest run');
});

test('loadAdapter throws on an unknown language (no such adapter ships)', () => {
  assert.throws(() => loadAdapter('rust'), /no adapter for language 'rust'/);
  // every advertised adapter language must actually load.
  for (const lang of ADAPTER_LANGS) assert.ok(loadAdapter(lang).lint, `${lang} adapter must have a lint command`);
});

// --- detectLanguages: extension -> adapter language over a real tree ---------

test('detectLanguages infers a sorted, de-duplicated language set from a mixed tree', () => {
  const dir = mkdtempSync(join(tmpdir(), 'adapters-detect-'));
  try {
    mkdirSync(join(dir, 'svc'), { recursive: true });
    writeFileSync(join(dir, 'main.go'), 'package main\n');
    writeFileSync(join(dir, 'svc', 'handler.go'), 'package svc\n'); // 2nd .go -> still one 'go'
    writeFileSync(join(dir, 'tool.mjs'), 'export const x = 1;\n');  // -> typescript
    writeFileSync(join(dir, 'app.ts'), 'export const y = 2;\n');    // -> typescript (deduped)
    writeFileSync(join(dir, 'script.py'), 'x = 1\n');               // -> python
    writeFileSync(join(dir, 'README.md'), '# not a source file\n'); // ignored
    const langs = detectLanguages(dir);
    assert.deepEqual(langs, ['go', 'python', 'typescript'], 'sorted + de-duplicated adapter languages');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('detectLanguages skips node_modules and returns [] for a source-free tree', () => {
  const dir = mkdtempSync(join(tmpdir(), 'adapters-empty-'));
  try {
    mkdirSync(join(dir, 'node_modules'), { recursive: true });
    writeFileSync(join(dir, 'node_modules', 'dep.js'), 'module.exports = {};\n'); // must be skipped
    writeFileSync(join(dir, 'notes.txt'), 'no source here\n');
    assert.deepEqual(detectLanguages(dir), []);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// --- pure helpers ------------------------------------------------------------

test('langForExt maps every JS/TS flavour to the single typescript adapter', () => {
  assert.equal(langForExt('.go'), 'go');
  assert.equal(langForExt('.py'), 'python');
  for (const ext of ['.mjs', '.cjs', '.js', '.jsx', '.ts', '.tsx']) {
    assert.equal(langForExt(ext), 'typescript', `${ext} -> typescript`);
  }
  assert.equal(langForExt('.md'), null, 'non-source extension -> null');
  assert.equal(langForExt('.rs'), null, 'no rust adapter -> null');
});

test('lintBinary extracts the executable (first token) of a lint command', () => {
  assert.equal(lintBinary('eslint . --max-warnings=0'), 'eslint');
  assert.equal(lintBinary('golangci-lint run ./...'), 'golangci-lint');
  assert.equal(lintBinary('ruff check .'), 'ruff');
  assert.equal(lintBinary('   spaced   args  '), 'spaced');
  assert.equal(lintBinary(undefined), null);
  assert.equal(lintBinary(''), null);
});

test('adapterCommands pulls {lint,test,coverage} run-strings; missing -> undefined', () => {
  const parsed = { commands: { lint: { run: 'L' }, test: { run: 'T' } } }; // no coverage
  assert.deepEqual(adapterCommands(parsed), { lint: 'L', test: 'T', coverage: undefined });
  assert.deepEqual(adapterCommands({}), { lint: undefined, test: undefined, coverage: undefined });
  assert.deepEqual(adapterCommands(null), { lint: undefined, test: undefined, coverage: undefined });
});

// --- judgeLint: the PURE honesty/fail-safe decision --------------------------

test('judgeLint -> PASS when the linter is installed and exited 0 (a real clean run)', () => {
  const v = judgeLint('typescript', 'eslint', true, { ok: true, code: 0, out: '' });
  assert.equal(v.status, PASS);
  assert.match(v.detail, /eslint clean/);
});

test('judgeLint -> FAIL when the linter is installed and reported real violations', () => {
  const v = judgeLint('typescript', 'eslint', true, { ok: false, code: 1, out: '3 problems (3 errors, 0 warnings)' });
  assert.equal(v.status, FAIL, 'real lint violations must FAIL');
  assert.match(v.detail, /eslint exit 1/);
});

test('judgeLint -> N/A when the linter is NOT installed (missing tool is never a FAIL)', () => {
  // The headline fail-safe: a guaranteed-absent linter yields N/A, not FAIL.
  const v = judgeLint('go', 'golangci-lint', false, null);
  assert.equal(v.status, NA, 'a missing linter must be N/A, never FAIL');
  assert.match(v.detail, /golangci-lint not installed/);
});

test('judgeLint -> N/A when installed but UNCONFIGURED (could not run; not a code verdict)', () => {
  // eslint with no config exits non-zero with this wording — not a verdict on the
  // code, so it must be N/A (never a faked PASS, never a FAIL on missing config).
  const noCfg = { ok: false, code: 2, out: "ESLint couldn't find a configuration file." };
  const v = judgeLint('typescript', 'eslint', true, noCfg);
  assert.equal(v.status, NA, 'unconfigured linter must be N/A, not FAIL');
  assert.match(v.detail, /unconfigured/);
});

test('judgeLint -> N/A when the adapter has no lint command (bin null)', () => {
  const v = judgeLint('go', null, false, null);
  assert.equal(v.status, NA);
  assert.match(v.detail, /no lint command/);
});

test('unconfigured matches can-not-run wording but NOT a normal violation report', () => {
  assert.equal(unconfigured("ESLint couldn't find a configuration file."), true);
  assert.equal(unconfigured('No configuration found'), true);
  assert.equal(unconfigured('5 problems (5 errors, 0 warnings)'), false, 'real violations are not "unconfigured"');
  assert.equal(unconfigured(''), false);
  assert.equal(unconfigured(undefined), false);
});

// --- probeLint over the REAL repo: honest aggregate verdict ------------------

test('probeLint returns a well-formed, honest result on the real repo', () => {
  const r = probeLint();
  assert.equal(r.criterion, 'lint');
  // Aggregate must be one of the three honest statuses (never a faked value).
  assert.ok([PASS, FAIL, NA].includes(r.status), `unexpected status ${r.status}`);
  // This repo ships no linter config, so no language can produce a real clean
  // run -> the criterion must NOT be a (faked) PASS, and a missing/unconfigured
  // linter must NOT be a FAIL. Honest outcome here is N/A.
  assert.equal(r.status, NA, `repo has no configured linter -> lint must be N/A (got ${r.status}: ${r.detail})`);
  assert.ok(r.detail.length > 0, 'N/A must carry an honest reason');
});

// === COVERAGE: the framework that mirrors lint (adapters.judgeCoverage) ======

// --- adapter coverage commands + first-token binary --------------------------

test('loadAdapter exposes each shipped coverage command from the yml maps', () => {
  assert.equal(loadAdapter('go').coverage, 'go test -coverprofile=coverage.out ./...');
  assert.equal(loadAdapter('python').coverage, 'pytest --cov --cov-report=json');
  assert.equal(loadAdapter('typescript').coverage, 'vitest run --coverage');
});

test('coverageBinary extracts the coverage tool (first token) of each command', () => {
  assert.equal(coverageBinary('go test -coverprofile=coverage.out ./...'), 'go');
  assert.equal(coverageBinary('pytest --cov --cov-report=json'), 'pytest');
  assert.equal(coverageBinary('vitest run --coverage'), 'vitest');
  assert.equal(coverageBinary(undefined), null, 'no coverage command -> null');
  assert.equal(coverageBinary(''), null);
});

test('coverageArtifact derives the report path each shipped coverage command writes', () => {
  // So the probe can delete a report IT created and not pollute the repo.
  assert.equal(coverageArtifact('go test -coverprofile=coverage.out ./...'), 'coverage.out');
  assert.equal(coverageArtifact('pytest --cov --cov-report=json'), 'coverage.json');
  assert.equal(coverageArtifact('vitest run --coverage'), 'coverage');
  // Commands that write no well-known artifact -> null (probe leaves tree alone).
  assert.equal(coverageArtifact('go test ./...'), null, 'no -coverprofile -> null');
  assert.equal(coverageArtifact(undefined), null);
  assert.equal(coverageArtifact(''), null);
});

// --- versionProbeArgs: the per-tool install probe (go is special) ------------

test('versionProbeArgs uses `go version` for go and `--version` otherwise', () => {
  // The honesty fix: `go --version` exits 2 ("flag not defined") and would read
  // as "go not installed" — go's version check is the subcommand `go version`.
  assert.deepEqual(versionProbeArgs('go'), ['version']);
  assert.deepEqual(versionProbeArgs('pytest'), ['--version']);
  assert.deepEqual(versionProbeArgs('vitest'), ['--version']);
  assert.deepEqual(versionProbeArgs('nyc'), ['--version']);
});

// --- coverageUnrunnable: can-not-run wording -> N/A, NOT a real low-cov FAIL --

test('coverageUnrunnable matches can-not-run signals but NOT a real coverage run', () => {
  // go from a non-module repo root (this repo's actual situation):
  assert.equal(coverageUnrunnable('pattern ./...: directory prefix . does not contain main module or its selected dependencies'), true);
  assert.equal(coverageUnrunnable('no test files'), true);
  assert.equal(coverageUnrunnable('FAIL\t./... [setup failed]'), true);
  // pytest / vitest can-not-run:
  assert.equal(coverageUnrunnable('no tests ran in 0.01s'), true);
  assert.equal(coverageUnrunnable('No test files found, exiting with code 1'), true);
  assert.equal(coverageUnrunnable('error: unrecognized arguments: --cov'), true);
  assert.equal(coverageUnrunnable('could not find config'), true);
  // A REAL coverage run that merely fell below threshold must NOT match (-> stays FAIL):
  assert.equal(coverageUnrunnable('coverage: 42.0% of statements'), false, 'a real low-coverage run is not "unrunnable"');
  assert.equal(coverageUnrunnable('TOTAL    120    30    75%'), false);
  assert.equal(coverageUnrunnable(''), false);
  assert.equal(coverageUnrunnable(undefined), false);
});

// --- parseCoveragePercent: pull the overall % from common tool outputs -------

test('parseCoveragePercent extracts the overall % (only %-signed figures; bare numbers -> null)', () => {
  // go: "coverage: 73.4% of statements".
  assert.equal(parseCoveragePercent('ok  \texample/pkg\t0.012s\tcoverage: 73.4% of statements'), 73.4);
  // coverage.py terminal summary: the TOTAL row is the overall figure.
  assert.equal(parseCoveragePercent('Name      Stmts   Miss  Cover\n-----\nTOTAL       120     18    85%'), 85);
  // A %-signed "All files" total row (e.g. a summary that prints the sign):
  assert.equal(parseCoveragePercent('All files: 91.2% lines covered'), 91.2);
  // HONESTY/fail-safe: we only trust figures carrying an explicit `%` sign — a
  // bare-number table row (istanbul-style "All files | 91.2 | 80 ...") is NOT
  // parsed (a bare number could be a line/branch count, not a %), so it returns
  // null -> judgeCoverage maps that to can-not-determine -> N/A, never a guess.
  assert.equal(parseCoveragePercent('All files |   91.2 |    80 |   88 |   91.2 |'), null, 'bare numbers (no %) -> null, not a guess');
  assert.equal(parseCoveragePercent('no percentage here'), null, 'no % -> null (can-not-determine)');
  assert.equal(parseCoveragePercent(''), null);
  assert.equal(parseCoveragePercent(undefined), null);
});

// --- judgeCoverage: the PURE honesty / fail-safe decision (no tool needed) ----

test('judgeCoverage -> PASS when the tool ran and % >= threshold', () => {
  const v = judgeCoverage('go', 'go', true, { ok: true, code: 0, out: 'coverage: 82.0% of statements' }, 60);
  assert.equal(v.status, PASS);
  assert.match(v.detail, /82.*>=.*60/);
});

test('judgeCoverage -> FAIL when the tool ran and % < threshold (a real low-cov verdict)', () => {
  const v = judgeCoverage('go', 'go', true, { ok: true, code: 0, out: 'coverage: 41.0% of statements' }, 60);
  assert.equal(v.status, FAIL, 'a real run below threshold must FAIL');
  assert.match(v.detail, /41.*<.*60/);
});

test('judgeCoverage uses DEFAULT_COVERAGE_THRESHOLD (60) when none is supplied', () => {
  assert.equal(DEFAULT_COVERAGE_THRESHOLD, 60);
  assert.equal(judgeCoverage('go', 'go', true, { ok: true, code: 0, out: 'coverage: 60.0% of statements' }).status, PASS, '== threshold is PASS');
  assert.equal(judgeCoverage('go', 'go', true, { ok: true, code: 0, out: 'coverage: 59.9% of statements' }).status, FAIL, 'just under default threshold FAILs');
});

test('judgeCoverage -> N/A when the tool is NOT installed (missing tool is never a FAIL)', () => {
  // The headline fail-safe: a guaranteed-absent coverage tool yields N/A, not FAIL.
  const v = judgeCoverage('python', 'pytest', false, null);
  assert.equal(v.status, NA, 'a missing coverage tool must be N/A, never FAIL');
  assert.match(v.detail, /pytest not installed/);
});

test('judgeCoverage -> N/A when installed but COULD NOT RUN here (not a code verdict)', () => {
  // go from a non-module repo root: installed, but `go test ./...` cannot run —
  // not a verdict on coverage, so N/A (never a FAIL, never a faked PASS).
  const r = { ok: false, code: 1, out: 'directory prefix . does not contain main module' };
  const v = judgeCoverage('go', 'go', true, r, 60);
  assert.equal(v.status, NA, 'an installed-but-unrunnable coverage tool must be N/A');
  assert.match(v.detail, /could not run here/);
});

test('judgeCoverage -> N/A when the tool ran but emitted no parseable % (can-not-determine)', () => {
  const r = { ok: true, code: 0, out: 'tests passed; (no coverage summary line)' };
  const v = judgeCoverage('typescript', 'vitest', true, r, 60);
  assert.equal(v.status, NA, 'no parseable % is can-not-determine -> N/A, not a faked PASS/FAIL');
  assert.match(v.detail, /no parseable coverage/);
});

test('judgeCoverage -> N/A when the adapter has no coverage command (bin null)', () => {
  const v = judgeCoverage('go', null, false, null);
  assert.equal(v.status, NA);
  assert.match(v.detail, /no coverage command/);
});

// --- probeCoverage over the REAL repo: honest aggregate verdict --------------

test('probeCoverage returns a well-formed, honest result on the real repo', () => {
  const r = probeCoverage();
  assert.equal(r.criterion, 'coverage');
  // Aggregate must be one of the three honest statuses (never a faked value).
  assert.ok([PASS, FAIL, NA].includes(r.status), `unexpected status ${r.status}`);
  // This repo wires no runnable+configured coverage tool (go is installed but the
  // repo root is not a Go module; pytest/vitest absent), so the honest outcome is
  // N/A — NOT a faked PASS, and a missing/unrunnable tool must NOT FAIL.
  assert.equal(r.status, NA, `repo has no runnable coverage tool -> coverage must be N/A (got ${r.status}: ${r.detail})`);
  assert.ok(r.detail.length > 0, 'N/A must carry an honest reason');
  // Honesty regression guard: go IS installed here, so the detail must NOT claim
  // "go not installed" (the `go --version` bug); it must say it could not run.
  assert.doesNotMatch(r.detail, /go not installed/, 'go is installed — detail must not say otherwise');
});

test('probeCoverage does NOT pollute the repo with a coverage.out artifact', () => {
  // The go coverage command drops `coverage.out` even when it fails; a gate must
  // not leave artifacts in the tree it judges. probeCoverage cleans up one it
  // created. (If a coverage.out already exists here, skip — never clobber it.)
  const repoRoot = dirname(dirname(fileURLToPath(import.meta.url))); // harness/.. = repo root
  const artifact = join(repoRoot, 'coverage.out');
  if (existsSync(artifact)) return; // pre-existing (not ours) — don't touch/assert.
  probeCoverage();
  assert.ok(!existsSync(artifact), 'probeCoverage must remove the coverage.out it created (no repo pollution)');
});
