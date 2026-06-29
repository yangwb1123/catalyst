// Tests for harness/select-tests.mjs — the INCREMENTAL test selector's PURE
// file->suite mapping (node:test, zero external deps). The mapping is the whole
// safety story: a governance OS must never let an incremental run silently "cover"
// a changed file it has no suite for, so the key invariants are (1) the right suite
// is selected per file kind, (2) distinct files dedup, and (3) a file with no mapped
// suite is reported UNMAPPED (so the runner recommends the full gate), never dropped.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mapFile, suitesForChanged } from './select-tests.mjs';

// exists predicate stubs for the sibling-test lookup (no filesystem needed).
const always = () => true;
const never = () => false;

test('mapFile: a changed TEST file runs ITSELF (the most direct signal)', () => {
  const mjs = mapFile('harness/test_gate.mjs', never);
  assert.equal(mjs.length, 1);
  assert.equal(mjs[0].cmd, 'node');
  assert.deepEqual(mjs[0].args, ['--test', '--test-reporter=tap', 'harness/test_gate.mjs']);

  const py = mapFile('harness/test_check.py', never);
  assert.equal(py.length, 1);
  assert.equal(py[0].cmd, 'python3');
  assert.ok(py[0].args[0].endsWith('harness/test_check.py'));

  // A nested arch test file runs itself too.
  const arch = mapFile('harness/arch/test_arch-check.mjs', never);
  assert.match(arch[0].args.at(-1), /harness\/arch\/test_arch-check\.mjs$/);
});

test('mapFile: a harness MODULE maps to its sibling test_<name> WHEN it exists', () => {
  // gate.mjs -> test_gate.mjs (present).
  const present = mapFile('harness/gate.mjs', always);
  assert.equal(present.length, 1);
  assert.match(present[0].label, /harness\/test_gate\.mjs$/);
  assert.equal(present[0].cmd, 'node');

  // arch/arch-check.mjs -> arch/test_arch-check.mjs (a sibling that EXISTS on disk).
  const arch = mapFile('harness/arch/arch-check.mjs', always);
  assert.match(arch[0].label, /harness\/arch\/test_arch-check\.mjs$/);

  // check.py -> test_check.py (present), run with python3.
  const py = mapFile('harness/check.py', always);
  assert.equal(py[0].cmd, 'python3');
  assert.match(py[0].label, /harness\/test_check\.py$/);
});

test('mapFile: a harness module with NO sibling test is UNMAPPED (honest, not guessed)', () => {
  // When the sibling test does not exist, return [] so the file is reported unmapped
  // and the runner recommends the full gate — never a partial/false cover.
  assert.deepEqual(mapFile('harness/gate.mjs', never), []);
});

test('mapFile: a changed Go file maps to `go test` on its PACKAGE dir (from forge-core)', () => {
  const m = mapFile('forge-core/internal/orchestrator/loop.go', never);
  assert.equal(m.length, 1);
  assert.equal(m[0].cmd, 'go');
  assert.deepEqual(m[0].args, ['test', './internal/orchestrator']);
  assert.match(m[0].cwd, /forge-core$/);

  // A cmd/forge file resolves to its own package.
  const cmd = mapFile('forge-core/cmd/forge/evolve.go', never);
  assert.deepEqual(cmd[0].args, ['test', './cmd/forge']);
});

test('mapFile: a governance asset maps to BOTH check.py and arch-check', () => {
  const agent = mapFile('.agent/workflows/build.yml', never);
  assert.equal(agent.length, 2);
  const labels = agent.map((s) => s.label).join(' ');
  assert.match(labels, /check\.py/);
  assert.match(labels, /arch-check/);

  // .arch rules changes also trip the architecture checks.
  const arch = mapFile('.arch/rules.yaml', never);
  assert.equal(arch.length, 2);
});

test('mapFile: an unrecognized path is UNMAPPED ([])', () => {
  assert.deepEqual(mapFile('README.md', never), []);
  assert.deepEqual(mapFile('docs/ignition.md', never), []);
});

test('suitesForChanged: DEDUPES suites and COLLECTS unmapped files (the full-gate trigger)', () => {
  // Modules chosen so the comments hold on disk: gate.mjs/adapters.mjs DO have sibling
  // test_*.mjs (the `always` stub here only isolates the pure formula from I/O).
  const files = [
    'harness/gate.mjs',               // -> test_gate.mjs
    'harness/adapters.mjs',           // -> test_adapters.mjs (a distinct suite)
    'harness/test_secret-scan.mjs',   // -> itself
    '.agent/project.yml',             // -> check.py + arch-check
    '.arch/rules.yaml',               // -> check.py + arch-check (DEDUP with above)
    'README.md',                      // -> UNMAPPED
    'docs/x.md',                      // -> UNMAPPED
  ];
  const { selected, unmapped } = suitesForChanged(files, always);
  // Unmapped are surfaced, never silently covered.
  assert.deepEqual(unmapped, ['README.md', 'docs/x.md']);
  // check.py + arch-check appear ONCE despite two governance files selecting them.
  const labels = selected.map((s) => s.label);
  assert.equal(labels.filter((l) => l.includes('check.py')).length, 1, 'check.py deduped');
  assert.equal(labels.filter((l) => l.includes('arch-check')).length, 1, 'arch-check deduped');
  // The two distinct module tests are both present.
  assert.ok(labels.some((l) => /test_gate\.mjs$/.test(l)));
  assert.ok(labels.some((l) => /test_adapters\.mjs$/.test(l)));
});

test('suitesForChanged: a changeset of only unmapped files selects NOTHING (recommend full gate)', () => {
  const { selected, unmapped } = suitesForChanged(['README.md', 'LICENSE'], always);
  assert.equal(selected.length, 0);
  assert.deepEqual(unmapped, ['README.md', 'LICENSE']);
});
