import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  chmodSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, rmSync,
  symlinkSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { WORK_INTENT_EXPECTED_FILES } from './work-intent-copy-fragment.mjs';
import {
  assertNoWorkIntentInstall,
  assertWorkIntentScaffold,
  WORK_INTENT_CONTRACT_TEST_ARGV,
  WORK_INTENT_FORBIDDEN_INSTALLS,
} from './work-intent-upgrade-verification.mjs';
import {
  renderScaffoldState, SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';
import { run } from './forge-upgrade.mjs';

const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const CORE = join('harness', 'work_intent_contract', 'codec.py');

function withAmbientPythonPoison(root, action) {
  const poison = join(root, 'python-poison');
  mkdirSync(poison);
  writeFileSync(join(poison, 'jsonschema.py'),
    'raise RuntimeError("ambient jsonschema poison imported")\n');
  const prior = {
    PYTHONPATH: process.env.PYTHONPATH,
    PYENV_VERSION: process.env.PYENV_VERSION,
  };
  process.env.PYTHONPATH = poison;
  process.env.PYENV_VERSION = 'work-intent-invalid-python-version';
  try {
    return action();
  } finally {
    for (const [name, value] of Object.entries(prior)) {
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
    }
  }
}

test('fresh WorkIntent v1 source-only scaffold is exact and fully verified', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'work-intent-fresh-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const result = spawnSync(process.execPath, [
    'harness/scaffold/forge-init.mjs', target, '--name', 'work-intent-focused',
  ], { cwd: SOURCE_ROOT, encoding: 'utf8' });
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.deepEqual(WORK_INTENT_CONTRACT_TEST_ARGV, [
    '-S', '-B', '-m', 'unittest', '-q',
    'harness.test_work_intent_contract_check',
  ]);
  withAmbientPythonPoison(root,
    () => assert.doesNotThrow(() => assertWorkIntentScaffold(target)));
  for (const relative of WORK_INTENT_EXPECTED_FILES) {
    assert.deepEqual(readFileSync(join(target, relative)),
      readFileSync(join(SOURCE_ROOT, relative)),
      `fresh WorkIntent copy drifted: ${relative}`);
  }

  const core = join(target, CORE);
  const coreBytes = readFileSync(core);
  writeFileSync(core, Buffer.concat([coreBytes, Buffer.from('\n# mutation\n')]));
  assert.throws(() => assertWorkIntentScaffold(target), /byte-identical to source/);
  writeFileSync(core, coreBytes);
  for (const relative of WORK_INTENT_EXPECTED_FILES) {
    const path = join(target, relative);
    chmodSync(path, 0o777);
    assert.throws(() => assertWorkIntentScaffold(target), /mode 0644/);
    chmodSync(path, 0o644);
  }
});

test('source-only boundary rejects runtime, Skill, route and authority state', (t) => {
  const target = mkdtempSync(join(tmpdir(), 'work-intent-negative-'));
  t.after(() => rmSync(target, { recursive: true, force: true }));
  const policy = join(target, POLICY);
  mkdirSync(dirname(policy), { recursive: true });
  const original = readFileSync(join(SOURCE_ROOT, POLICY));
  writeFileSync(policy, original);
  assert.doesNotThrow(() => assertNoWorkIntentInstall(target));

  for (const parts of WORK_INTENT_FORBIDDEN_INSTALLS) {
    const forbidden = join(target, ...parts);
    mkdirSync(forbidden, { recursive: true });
    assert.throws(() => assertNoWorkIntentInstall(target), /must not install/);
    rmSync(forbidden, { recursive: true, force: true });
  }

  const authority = join(target, '.forge', 'authority');
  mkdirSync(dirname(authority), { recursive: true });
  writeFileSync(authority, 'forbidden\n');
  assert.throws(() => assertNoWorkIntentInstall(target), /must not install/);
  rmSync(authority);
  symlinkSync('missing-authority-target', authority);
  assert.throws(() => assertNoWorkIntentInstall(target), /must not install/);
  rmSync(authority);
  const fifo = spawnSync('mkfifo', [authority], { encoding: 'utf8' });
  assert.equal(fifo.status, 0, fifo.stderr);
  assert.throws(() => assertNoWorkIntentInstall(target), /must not install/);
  rmSync(authority);

  const text = original.toString('utf8');
  for (const mutation of [
    text.replace('version: 39', 'version: 35'),
    text.replace('    semantic_authority: false', '    semantic_authority: true'),
    text.replace('    g0_closure: false', '    g0_closure: true'),
    text.replace('    persistence_execution_or_effect: false',
      '    persistence_execution_or_effect: true'),
  ]) {
    writeFileSync(policy, mutation);
    assert.throws(() => assertNoWorkIntentInstall(target));
  }
});

test('legacy upgrade restores exact fourteen files and ledger but not mode drift', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'work-intent-legacy-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const initialized = spawnSync(process.execPath, [
    'harness/scaffold/forge-init.mjs', target, '--name', 'work-intent-legacy',
  ], { cwd: SOURCE_ROOT, encoding: 'utf8' });
  assert.equal(initialized.status, 0, initialized.stderr);
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  const legacy = state.copied.filter(
    (relative) => !WORK_INTENT_EXPECTED_FILES.includes(relative));
  writeFileSync(statePath, renderScaffoldState(legacy));
  for (const relative of WORK_INTENT_EXPECTED_FILES) {
    rmSync(join(target, relative), { force: true });
  }

  const restored = run({
    from: SOURCE_ROOT, target, apply: true, backup: true, prune: false,
  });
  assert.deepEqual([...restored.drift.added].sort(),
    [...WORK_INTENT_EXPECTED_FILES].sort());
  assert.doesNotThrow(() => assertWorkIntentScaffold(target));

  const core = join(target, CORE);
  chmodSync(core, 0o777);
  const unchanged = run({
    from: SOURCE_ROOT, target, apply: true, backup: true, prune: false,
  });
  assert.equal(unchanged.drift.unchanged.includes(CORE), true);
  assert.equal(lstatSync(core).mode & 0o777, 0o777,
    'generic byte upgrade must not be misrepresented as mode repair');
  assert.throws(() => assertWorkIntentScaffold(target),
    /mode 0644; operator remediation required/);
  chmodSync(core, 0o644);
  assert.doesNotThrow(() => assertWorkIntentScaffold(target));
});
