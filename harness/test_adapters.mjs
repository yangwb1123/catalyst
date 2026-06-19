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
import { mkdtempSync, rmSync, writeFileSync, mkdirSync, existsSync, readFileSync } from 'node:fs';
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
  COVERAGE_THRESHOLD_CAP,
  computeCoverageThreshold,
  resolveCoverageThreshold,
  appTestPlan,
  testCmdMatchesLayout,
  ADAPTER_LANG_BY_RUNNER,
} from './adapters.mjs';
import { judgeLint, unconfigured, probeLint, probeCoverage, PASS, FAIL, NA } from './acceptance.mjs';
import { parseRules } from './arch/scan.mjs';

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

// === APP TEST SELECTION: declaration-driven runner with honest fallback ======
// These pin the gap-closing behavior: the app-test runner now PREFERS the
// adapter `test:` command and falls back to the hardcoded runner only when that
// command's runner does not fit the app's layout — annotating WHY.

test('ADAPTER_LANG_BY_RUNNER maps each inferRunner kind onto its <lang>.yml', () => {
  // node funnels to the typescript adapter (same as LANG_BY_EXT / detectLanguages).
  assert.deepEqual(ADAPTER_LANG_BY_RUNNER, { node: 'typescript', python: 'python', go: 'go' });
});

test('testCmdMatchesLayout: a runner fits only when the app files match its convention', () => {
  // go test fits *_test.go; node:test fits *.test.mjs.
  assert.equal(testCmdMatchesLayout('go', ['domain_test.go', 'go.mod']), true);
  assert.equal(testCmdMatchesLayout('node', ['url.test.mjs']), true);
  assert.equal(testCmdMatchesLayout('pytest', ['test_foo.py']), true, 'test_ prefix rule');
  assert.equal(testCmdMatchesLayout('pytest', ['foo_test.py']), true, '_test.py suffix rule');
  // The headline mismatch: vitest does NOT discover node:test *.test.mjs.
  assert.equal(testCmdMatchesLayout('vitest', ['url.test.mjs']), false, 'vitest != node:test *.test.mjs');
  assert.equal(testCmdMatchesLayout('go', ['url.test.mjs']), false, 'go test does not fit a node app');
  assert.equal(testCmdMatchesLayout('unknown-runner', ['x_test.go']), false, 'unknown runner never fits');
  assert.equal(testCmdMatchesLayout('node', []), false, 'no files -> no fit');
});

test('appTestPlan -> ADAPTER path when the adapter test command fits (go-taskd)', () => {
  // go-taskd's *_test.go layout fits `go test ./...`: use the DECLARED command,
  // run from the app's own module dir (relative ./... -> cwd = examples/<app>).
  const plan = appTestPlan('go test ./...', ['domain_test.go'], 'go-taskd', 'go');
  assert.equal(plan.cmd, 'go');
  assert.deepEqual(plan.args, ['test', './...']);
  assert.equal(plan.cwd, 'examples/go-taskd', 'relative ./... command runs from the module dir');
  assert.equal(plan.countCheck, false, 'go judges on exit code, not the node tests-count');
  assert.match(plan.tag, /^adapter: go test \.\/\.\.\.$/);
});

test('appTestPlan -> node FALLBACK (with honest reason) when vitest does not fit *.test.mjs', () => {
  // url-shortener: the typescript adapter ships `vitest run`, which does not
  // discover node:test *.test.mjs -> fall back to the node counting runner, and
  // SAY WHY (honesty). node carries no cmd here (acceptance.mjs's runNodeApp owns
  // the *.test.mjs glob + the fail-closed count); countCheck flags that path.
  const plan = appTestPlan('vitest run', ['http.test.mjs', 'url.test.mjs'], 'url-shortener', 'node');
  assert.equal(plan.countCheck, true, 'node fallback must route to the counting runner');
  assert.equal(plan.cmd, null, 'node command is resolved by acceptance.mjs, not the plan');
  assert.match(plan.tag, /node fallback: adapter test runner 'vitest' does not fit app layout/);
});

test('appTestPlan -> python FALLBACK command when pytest does not fit a unittest layout', () => {
  // Forward-looking (no python example ships yet): a unittest-style test_*.py app
  // DOES match pytest's convention, so that fits -> adapter path. But an app whose
  // files match NO pytest convention falls back to `python -m unittest discover`.
  const fits = appTestPlan('pytest -q', ['test_app.py'], 'pyapp', 'python');
  assert.equal(fits.cmd, 'pytest', 'a test_*.py app fits the pytest adapter command');
  assert.match(fits.tag, /^adapter: pytest -q$/);
  const fall = appTestPlan('pytest -q', ['weird.py'], 'pyapp', 'python');
  assert.deepEqual([fall.cmd, ...fall.args], ['python3', '-m', 'unittest', 'discover', '-s', 'examples/pyapp/test']);
  assert.match(fall.tag, /python fallback:/);
});

test('appTestPlan -> fallback with "no test command" when the adapter ships none', () => {
  const plan = appTestPlan(undefined, ['x_test.go'], 'someapp', 'go');
  assert.match(plan.tag, /go fallback: adapter has no test command/);
  assert.deepEqual([plan.cmd, ...plan.args], ['go', '-C', 'examples/someapp', 'test', './...']);
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

// === COVERAGE THRESHOLD: mode×lifecycle resolution (central knob -> floor) ====
// The gap this closes: the coverage criterion's threshold was hardcoded 60,
// ignoring the project's mode (explorer 0 / balanced 60 / engineering 80) and
// lifecycle modifier (idea 0 / growth +10 / production +20, cap 95). These pin the
// arithmetic, the +N-string coercion the minimal YAML reader forces, and the
// fail-safe fallback to the default when a field/file is missing or malformed.

// A modes object shaped exactly like parseRules(modes.yml) returns — including the
// quirk that `coverage_delta: +10` parses as the STRING "+10" (the reader's number
// coercion is `-?\d+` only), so computeCoverageThreshold must Number()-coerce it.
const MODES_FIXTURE = {
  modes: {
    explorer: { harness: { coverage_threshold: 0 } },
    balanced: { harness: { coverage_threshold: 60 } },
    engineering: { harness: { coverage_threshold: 80 } },
  },
  lifecycle_modifiers: {
    idea: { coverage_delta: 0 },
    mvp: { coverage_delta: 0 },
    growth: { coverage_delta: '+10' },     // the leading-`+` STRING the reader yields
    production: { coverage_delta: '+20' },
  },
};

test('computeCoverageThreshold: base per mode (explorer 0 / balanced 60 / engineering 80)', () => {
  // idea's delta is 0, so these isolate the per-mode BASE.
  assert.equal(computeCoverageThreshold(MODES_FIXTURE, 'explorer', 'idea'), 0);
  assert.equal(computeCoverageThreshold(MODES_FIXTURE, 'balanced', 'idea'), 60);
  assert.equal(computeCoverageThreshold(MODES_FIXTURE, 'engineering', 'idea'), 80);
});

test('computeCoverageThreshold: lifecycle delta adds onto the base (the +N modifiers)', () => {
  // balanced(60) + production(+20) = 80; explorer(0) + growth(+10) = 10.
  assert.equal(computeCoverageThreshold(MODES_FIXTURE, 'balanced', 'production'), 80);
  assert.equal(computeCoverageThreshold(MODES_FIXTURE, 'explorer', 'growth'), 10);
  assert.equal(computeCoverageThreshold(MODES_FIXTURE, 'balanced', 'growth'), 70);
  // The +N coercion is the load-bearing bit: the string "+10" must add as 10.
  assert.equal(computeCoverageThreshold(MODES_FIXTURE, 'engineering', 'growth'), 90);
});

test('computeCoverageThreshold: clamps at the 95 cap (engineering + production -> 95, not 100)', () => {
  assert.equal(COVERAGE_THRESHOLD_CAP, 95);
  assert.equal(computeCoverageThreshold(MODES_FIXTURE, 'engineering', 'production'), 95, '80+20 caps at 95');
});

test('computeCoverageThreshold: FAIL-SAFE to default on missing mode/lifecycle/field', () => {
  // Unknown mode -> base is undefined -> NaN -> fallback (never a guess).
  assert.equal(computeCoverageThreshold(MODES_FIXTURE, 'cto', 'idea'), DEFAULT_COVERAGE_THRESHOLD, 'no harness.coverage_threshold (cto absent in fixture) -> default');
  assert.equal(computeCoverageThreshold(MODES_FIXTURE, 'explorer', 'nope'), DEFAULT_COVERAGE_THRESHOLD, 'unknown lifecycle -> default');
  assert.equal(computeCoverageThreshold(MODES_FIXTURE, undefined, undefined), DEFAULT_COVERAGE_THRESHOLD, 'no mode/lifecycle -> default');
  assert.equal(computeCoverageThreshold({}, 'balanced', 'mvp'), DEFAULT_COVERAGE_THRESHOLD, 'empty modes -> default');
  assert.equal(computeCoverageThreshold(null, 'balanced', 'mvp'), DEFAULT_COVERAGE_THRESHOLD, 'null modes -> default');
  // A non-numeric coverage_threshold (e.g. a stray string) -> NaN -> default.
  const bad = { modes: { balanced: { harness: { coverage_threshold: 'sixty' } } }, lifecycle_modifiers: { mvp: { coverage_delta: 0 } } };
  assert.equal(computeCoverageThreshold(bad, 'balanced', 'mvp'), DEFAULT_COVERAGE_THRESHOLD, 'non-numeric base -> default');
});

// resolveCoverageThreshold: the I/O boundary. writeAgent stages a temp root with
// a chosen project.yml against the REPO'S OWN modes.yml (copied in) so the on-disk
// path exercises the real policy file — proving the `+10`/`+20` strings round-trip
// from disk through parseRules to numbers, and the missing-file fail-safe.
const REPO_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const REAL_MODES = readFileSync(join(REPO_ROOT, '.agent', 'policies', 'modes.yml'), 'utf8');
function writeAgent(root, projectYml, modesYml = REAL_MODES) {
  mkdirSync(join(root, '.agent', 'policies'), { recursive: true });
  if (projectYml !== null) writeFileSync(join(root, '.agent', 'project.yml'), projectYml);
  if (modesYml !== null) writeFileSync(join(root, '.agent', 'policies', 'modes.yml'), modesYml);
}
const inTmp = (fn) => { const d = mkdtempSync(join(tmpdir(), 'cov-thr-')); try { return fn(d); } finally { rmSync(d, { recursive: true, force: true }); } };

test('resolveCoverageThreshold reads project.yml × modes.yml off disk (the mode×lifecycle chain)', () => {
  // engineering×mvp -> 80+0 = 80 (mirrors this repo); explorer×mvp -> 0; and
  // balanced×production -> 60+20 = 80, proving the on-disk `+20` string coerces.
  assert.equal(inTmp((d) => { writeAgent(d, 'mode: engineering\nlifecycle: mvp\n'); return resolveCoverageThreshold(d); }), 80);
  assert.equal(inTmp((d) => { writeAgent(d, 'mode: explorer\nlifecycle: mvp\n'); return resolveCoverageThreshold(d); }), 0);
  assert.equal(inTmp((d) => { writeAgent(d, 'mode: balanced\nlifecycle: production\n'); return resolveCoverageThreshold(d); }), 80, 'balanced+production = 60+20, +20 string coerced off disk');
  // explorer×production = 0+20 = 20, and engineering×production caps 80+20 -> 95.
  assert.equal(inTmp((d) => { writeAgent(d, 'mode: explorer\nlifecycle: production\n'); return resolveCoverageThreshold(d); }), 20);
  assert.equal(inTmp((d) => { writeAgent(d, 'mode: engineering\nlifecycle: production\n'); return resolveCoverageThreshold(d); }), 95, 'cap at 95');
});

test('resolveCoverageThreshold: FAIL-SAFE to 60 when project.yml or modes.yml is missing', () => {
  assert.equal(inTmp((d) => resolveCoverageThreshold(d)), DEFAULT_COVERAGE_THRESHOLD, 'no .agent at all -> default 60');
  assert.equal(inTmp((d) => { writeAgent(d, 'mode: engineering\nlifecycle: mvp\n', null); return resolveCoverageThreshold(d); }), DEFAULT_COVERAGE_THRESHOLD, 'modes.yml missing -> default 60');
  assert.equal(inTmp((d) => { writeAgent(d, null); return resolveCoverageThreshold(d); }), DEFAULT_COVERAGE_THRESHOLD, 'project.yml missing -> default 60');
});

test('resolveCoverageThreshold on THIS repo agrees with computeCoverageThreshold over its OWN mode×lifecycle', () => {
  // The live wire — host-AGNOSTIC (this test ships VERBATIM to every scaffolded
  // project, which may be ANY mode×lifecycle, not necessarily engineering×mvp).
  // Rather than hardcode the host's number, read the host's OWN project.yml ×
  // modes.yml and assert the on-disk resolve equals the pure computation for that
  // exact pair. This proves the I/O boundary wires the central knob through (not
  // the old hardcoded floor) without assuming WHICH knob the host is set to.
  const project = parseRules(readFileSync(join(REPO_ROOT, '.agent', 'project.yml'), 'utf8'));
  const modes = parseRules(REAL_MODES);
  const expected = computeCoverageThreshold(modes, project.mode, project.lifecycle);
  assert.equal(resolveCoverageThreshold(REPO_ROOT), expected, `this repo (${project.mode}×${project.lifecycle}) resolves to its computed floor ${expected}`);
});
// NOTE: the sibling ENFORCE-strictness resolution exported by adapters.mjs (warn|
// block knob) is unit-tested in test_enforce.mjs; gate end-to-end in test_gate.mjs.
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
