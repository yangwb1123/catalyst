// Tests for harness/forge-init.mjs (node:test, zero external deps).
// Run: node --test harness/test_forge-init.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';

const HARNESS_DIR = dirname(fileURLToPath(import.meta.url));
const SOURCE_ROOT = dirname(HARNESS_DIR);
const INIT_PATH = join(HARNESS_DIR, 'forge-init.mjs');

// Run forge-init as a child process; returns spawnSync result.
function runInit(args) {
  return spawnSync(process.execPath, [INIT_PATH, ...args], { encoding: 'utf8' });
}

// The host-independent ENFORCERS a fresh project must inherit verbatim — each only
// scans files / resolves paths from its own location, so it runs with zero project
// wiring. .arch/rules.yaml ships with policies.yml because arch-check's drift-guard
// asserts the two agree.
const COPIED_ENFORCERS = [
  join('.agent', 'AGENTS.md'),
  join('harness', 'gate.mjs'),
  join('harness', 'policies.yml'),
  join('harness', 'arch', 'arch-check.mjs'),
  join('harness', 'arch', 'scan.mjs'),
  join('harness', 'arch', 'scan-functions.mjs'),
  join('harness', 'secret-scan.mjs'),
  join('.arch', 'rules.yaml'),
];

const EXPECTED_FILES = [
  ...COPIED_ENFORCERS,
  join('.agent', 'PROJECT.md'),
  join('.agent', 'ROADMAP.md'),
  join('.agent', 'CURRENT_SPRINT.md'),
  join('.agent', 'project.yml'),
  'README.md',
  '.gitignore',
];

test('forge-init scaffolds a fresh, governed, runnable project', (t) => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-init-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const target = join(dir, 'proj');

  const res = runInit([target, '--name', 'acme-svc']);
  assert.equal(res.status, 0, `init exit 0 expected; stderr:\n${res.stderr}`);

  // (1) every expected file exists.
  for (const rel of EXPECTED_FILES) {
    assert.ok(existsSync(join(target, rel)), `missing scaffolded file: ${rel}`);
  }

  // (2) project.yml + PROJECT.md carry the --name.
  assert.match(readFileSync(join(target, '.agent', 'project.yml'), 'utf8'), /acme-svc/);
  assert.match(readFileSync(join(target, '.agent', 'PROJECT.md'), 'utf8'), /acme-svc/);

  // (3) every copied ENFORCER (incl. the red-lines AGENTS.md) is byte-identical to
  // its source — a fresh project inherits the REAL tools verbatim, not a fork.
  for (const rel of COPIED_ENFORCERS) {
    assert.deepEqual(
      readFileSync(join(target, rel)),
      readFileSync(join(SOURCE_ROOT, rel)),
      `copied ${rel} must be byte-identical to source`,
    );
  }

  // (4) ★ running the FULL inherited enforcement triad on the fresh project all
  // PASSES out of the box — the iron proof that a fresh project inherits WORKING,
  // runnable governance (not just files on disk). A fresh project has no business
  // source, so arch-check's layering/package/fanin/cognitive/naming/function-length/
  // circular/drift-guard report NO violations and secret-scan finds 0 secrets.
  const enforcers = [
    { name: 'gate', script: join('harness', 'gate.mjs'), ok: /forge-gate: PASS/ },
    { name: 'arch-check', script: join('harness', 'arch', 'arch-check.mjs'), ok: /forge-arch: PASS/ },
    { name: 'secret-scan', script: join('harness', 'secret-scan.mjs'), ok: /forge-secret-scan: PASS/ },
  ];
  for (const e of enforcers) {
    // cwd: target — gate.mjs roots at process.cwd(); the others root at their own
    // on-disk location, so cwd is harmless to them and keeps the call uniform.
    const r = spawnSync(process.execPath, [join(target, e.script)], { cwd: target, encoding: 'utf8' });
    assert.equal(r.status, 0, `copied ${e.name} must PASS out of the box; stdout:\n${r.stdout}\nstderr:\n${r.stderr}`);
    assert.match(r.stdout, e.ok, `${e.name} stdout must report PASS`);
  }
});

test('forge-init refuses to clobber a non-empty .agent without --force; --force succeeds', (t) => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-init-force-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const target = join(dir, 'proj');

  const first = runInit([target, '--name', 'acme-svc']);
  assert.equal(first.status, 0, `first init exit 0; stderr:\n${first.stderr}`);

  // (5) re-init without --force is refused (non-zero) when .agent exists.
  const reinit = runInit([target, '--name', 'acme-svc']);
  assert.notEqual(reinit.status, 0, 're-init without --force must fail');
  assert.match(reinit.stderr, /--force/);

  // ...and --force succeeds.
  const forced = runInit([target, '--name', 'acme-svc', '--force']);
  assert.equal(forced.status, 0, `--force re-init must succeed; stderr:\n${forced.stderr}`);
});

test('forge-init exits non-zero on missing required args', () => {
  const noName = runInit([join(tmpdir(), 'forge-init-noname')]);
  assert.equal(noName.status, 2, 'missing --name must exit 2');
  assert.match(noName.stderr, /--name/);

  const noTarget = runInit(['--name', 'x']);
  assert.equal(noTarget.status, 2, 'missing <target-dir> must exit 2');
  assert.match(noTarget.stderr, /target-dir/);
});
