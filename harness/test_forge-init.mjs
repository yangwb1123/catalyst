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
const GATE_PATH = join(HARNESS_DIR, 'gate.mjs');

// Run forge-init as a child process; returns spawnSync result.
function runInit(args) {
  return spawnSync(process.execPath, [INIT_PATH, ...args], { encoding: 'utf8' });
}

const EXPECTED_FILES = [
  join('.agent', 'AGENTS.md'),
  join('.agent', 'PROJECT.md'),
  join('.agent', 'ROADMAP.md'),
  join('.agent', 'CURRENT_SPRINT.md'),
  join('.agent', 'project.yml'),
  join('harness', 'gate.mjs'),
  join('harness', 'policies.yml'),
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

  // (3) copied gate.mjs is byte-identical to the source.
  assert.deepEqual(
    readFileSync(join(target, 'harness', 'gate.mjs')),
    readFileSync(GATE_PATH),
    'copied gate.mjs must be byte-identical to source',
  );
  // The red-lines (.agent/AGENTS.md) travel verbatim too.
  assert.deepEqual(
    readFileSync(join(target, '.agent', 'AGENTS.md')),
    readFileSync(join(SOURCE_ROOT, '.agent', 'AGENTS.md')),
    'copied AGENTS.md must be byte-identical to source',
  );

  // (4) ★ running the COPIED gate on the new project PASSES — proves a fresh
  // project inherits WORKING governance, runnable immediately.
  const gate = spawnSync(process.execPath, [join(target, 'harness', 'gate.mjs')], {
    cwd: target,
    encoding: 'utf8',
  });
  assert.equal(gate.status, 0, `copied gate must PASS; stdout:\n${gate.stdout}\nstderr:\n${gate.stderr}`);
  assert.match(gate.stdout, /forge-gate: PASS/);
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
