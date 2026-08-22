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

import {
  AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES,
} from './authenticated-adr-approval-copy-fragment.mjs';
import {
  assertAuthenticatedADRApprovalScaffold,
  assertNoAuthenticatedADRApprovalInstall,
  AUTHENTICATED_ADR_APPROVAL_CONTRACT_TEST_ARGV,
  AUTHENTICATED_ADR_APPROVAL_FORBIDDEN_INSTALLS,
} from './authenticated-adr-approval-upgrade-verification.mjs';
import {
  renderScaffoldState, SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';
import { run } from './forge-upgrade.mjs';

const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const CORE = join('harness', 'authenticated_adr_approval_contract', 'contract.py');

function withAmbientPythonPoison(root, action) {
  const poison = join(root, 'python-poison');
  mkdirSync(poison);
  writeFileSync(join(poison, 'jsonschema.py'),
    'raise RuntimeError("ambient jsonschema poison imported")\n');
  writeFileSync(join(poison, 'referencing.py'),
    'raise RuntimeError("ambient referencing poison imported")\n');
  const prior = {
    PYTHONPATH: process.env.PYTHONPATH,
    PYENV_VERSION: process.env.PYENV_VERSION,
  };
  process.env.PYTHONPATH = poison;
  process.env.PYENV_VERSION = 'authenticated-adr-approval-invalid-python-version';
  try {
    return action();
  } finally {
    for (const [name, value] of Object.entries(prior)) {
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
    }
  }
}

function initialize(target, name) {
  return spawnSync(process.execPath, [
    'harness/scaffold/forge-init.mjs', target, '--name', name,
  ], { cwd: SOURCE_ROOT, encoding: 'utf8' });
}

test('fresh authenticated ADR approval source-only scaffold is exact', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'authenticated-adr-approval-fresh-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const result = initialize(target, 'authenticated-adr-approval-focused');
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.deepEqual(AUTHENTICATED_ADR_APPROVAL_CONTRACT_TEST_ARGV, [
    '-S', '-B', '-m', 'unittest', '-q',
    'harness.test_authenticated_adr_approval_contract',
  ]);
  withAmbientPythonPoison(root,
    () => assert.doesNotThrow(() => assertAuthenticatedADRApprovalScaffold(target)));
  for (const relative of AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES) {
    assert.deepEqual(readFileSync(join(target, relative)),
      readFileSync(join(SOURCE_ROOT, relative)),
      `fresh authenticated ADR approval copy drifted: ${relative}`);
  }

  const core = join(target, CORE);
  const coreBytes = readFileSync(core);
  writeFileSync(core, Buffer.concat([coreBytes, Buffer.from('\n# mutation\n')]));
  assert.throws(() => assertAuthenticatedADRApprovalScaffold(target),
    /byte-identical to source/);
  writeFileSync(core, coreBytes);
  for (const relative of AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES) {
    const path = join(target, relative);
    chmodSync(path, 0o777);
    assert.throws(() => assertAuthenticatedADRApprovalScaffold(target), /mode 0644/);
    chmodSync(path, 0o644);
  }
});

test('source-only boundary rejects service, keys, state, Skill and route nodes', (t) => {
  const target = mkdtempSync(join(tmpdir(), 'authenticated-adr-approval-negative-'));
  t.after(() => rmSync(target, { recursive: true, force: true }));
  const policy = join(target, POLICY);
  mkdirSync(dirname(policy), { recursive: true });
  const original = readFileSync(join(SOURCE_ROOT, POLICY));
  writeFileSync(policy, original);
  assert.doesNotThrow(() => assertNoAuthenticatedADRApprovalInstall(target));

  for (const parts of AUTHENTICATED_ADR_APPROVAL_FORBIDDEN_INSTALLS) {
    const forbidden = join(target, ...parts);
    mkdirSync(forbidden, { recursive: true });
    assert.throws(() => assertNoAuthenticatedADRApprovalInstall(target),
      /must not install/);
    rmSync(forbidden, { recursive: true, force: true });
  }

  const authority = join(target, '.forge', 'authority');
  mkdirSync(dirname(authority), { recursive: true });
  writeFileSync(authority, 'forbidden\n');
  assert.throws(() => assertNoAuthenticatedADRApprovalInstall(target),
    /must not install/);
  rmSync(authority);
  symlinkSync('missing-authority-target', authority);
  assert.throws(() => assertNoAuthenticatedADRApprovalInstall(target),
    /must not install/);
  rmSync(authority);
  const fifo = spawnSync('mkfifo', [authority], { encoding: 'utf8' });
  assert.equal(fifo.status, 0, fifo.stderr);
  assert.throws(() => assertNoAuthenticatedADRApprovalInstall(target),
    /must not install/);
  rmSync(authority);

  const text = original.toString('utf8');
  for (const mutation of [
    text.replace('version: 39', 'version: 35'),
    text.replace('    ed25519_or_sod_proof_verification: false',
      '    ed25519_or_sod_proof_verification: true'),
    text.replace('    authorization: false', '    authorization: true'),
    text.replace('    adr_acceptance_or_lifecycle_transition: false',
      '    adr_acceptance_or_lifecycle_transition: true'),
    text.replace('    copies_go_contract_or_authority: false',
      '    copies_go_contract_or_authority: true'),
  ]) {
    writeFileSync(policy, mutation);
    assert.throws(() => assertNoAuthenticatedADRApprovalInstall(target));
  }
});

test('legacy upgrade restores exact 22 files and ledger but not mode drift', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'authenticated-adr-approval-legacy-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const initialized = initialize(target, 'authenticated-adr-approval-legacy');
  assert.equal(initialized.status, 0, `${initialized.stdout}\n${initialized.stderr}`);
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  const legacy = state.copied.filter(
    (relative) => !AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES.includes(relative));
  writeFileSync(statePath, renderScaffoldState(legacy));
  for (const relative of AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES) {
    rmSync(join(target, relative), { force: true });
  }

  const restored = run({
    from: SOURCE_ROOT, target, apply: true, backup: true, prune: false,
  });
  assert.deepEqual([...restored.drift.added].sort(),
    [...AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES].sort());
  assert.doesNotThrow(() => assertAuthenticatedADRApprovalScaffold(target));
  const recorded = JSON.parse(readFileSync(statePath, 'utf8')).copied;
  for (const relative of AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES) {
    assert.equal(recorded.includes(relative), true, `ledger missing ${relative}`);
  }

  const core = join(target, CORE);
  chmodSync(core, 0o777);
  const unchanged = run({
    from: SOURCE_ROOT, target, apply: true, backup: true, prune: false,
  });
  assert.equal(unchanged.drift.unchanged.includes(CORE), true);
  assert.equal(lstatSync(core).mode & 0o777, 0o777,
    'generic byte upgrade must not be misrepresented as mode repair');
  assert.throws(() => assertAuthenticatedADRApprovalScaffold(target),
    /mode 0644; operator remediation required/);
  chmodSync(core, 0o644);
  assert.doesNotThrow(() => assertAuthenticatedADRApprovalScaffold(target));
});
