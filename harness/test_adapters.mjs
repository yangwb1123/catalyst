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
import { mkdtempSync, rmSync, writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

import {
  loadAdapter,
  detectLanguages,
  langForExt,
  lintBinary,
  adapterCommands,
  ADAPTER_LANGS,
} from './adapters.mjs';
import { judgeLint, unconfigured, probeLint, PASS, FAIL, NA } from './acceptance.mjs';

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
