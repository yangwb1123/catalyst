import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  chmodSync, copyFileSync, linkSync, lstatSync, mkdirSync, mkdtempSync,
  readFileSync, rmSync, symlinkSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
} from './kernel-decision-reference-copy-fragment.mjs';
import {
  KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES,
} from './kernel-operational-reference-copy-fragment.mjs';
import {
  assertKernelOperationalReferenceProjection,
  assertKernelOperationalReferenceScaffold,
  assertNoKernelOperationalReferenceInstall,
} from './kernel-operational-reference-upgrade-verification.mjs';
import {
  assertRegistryV36Projection, REGISTRY_V37_OWNER_FILES,
  seedRegistryV36Projection,
} from './kernel-operational-reference-v36-projection.mjs';
import {
  REGISTRY_V39_SHARED_OWNER_FILES,
} from './decision-capsule-structural-replay-v38-projection.mjs';
import {
  DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES,
} from './decision-capsule-structural-replay-copy-fragment.mjs';
import {
  renderScaffoldState, SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';
import { run } from './forge-upgrade.mjs';

const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const DETECTORS = join('.agent', 'engineering', 'detectors.yml');
const PACKAGE = join('harness', 'kernel_operational_contract');
const MUTATION_TARGET = join('harness', 'governance_engineering',
  'kernel_operational_reference_candidate.py');
const UPGRADE_ADDED_FILES = [
  ...KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES,
  ...KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
  ...DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES,
];
const UPGRADE_CHANGED_FILES = [...new Set([
  ...REGISTRY_V37_OWNER_FILES, ...REGISTRY_V39_SHARED_OWNER_FILES,
])].filter((relative) => !UPGRADE_ADDED_FILES.includes(relative));

function initializedProject(t, prefix, name) {
  const root = mkdtempSync(join(tmpdir(), prefix));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const result = spawnSync(process.execPath, [
    'harness/scaffold/forge-init.mjs', target, '--name', name,
  ], { cwd: SOURCE_ROOT, encoding: 'utf8' });
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  return target;
}

function restoreExact(target, relative) {
  const path = join(target, relative);
  rmSync(path, { force: true });
  copyFileSync(join(SOURCE_ROOT, relative), path);
  chmodSync(path, 0o644);
}

function state(target) {
  return JSON.parse(readFileSync(join(target, SCAFFOLD_STATE_FILE), 'utf8'));
}

function writeState(target, copied) {
  writeFileSync(join(target, SCAFFOLD_STATE_FILE), renderScaffoldState(copied));
}

test('exact18 is closed, Python-only and excludes Catalyst Go/Rust parity', () => {
  const files = KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES;
  assert.equal(files.length, 18);
  assert.equal(new Set(files).size, 18);
  assert.equal(files.filter((relative) => relative.includes('ADR-0088')).length, 1);
  assert.equal(files.filter((relative) => relative.includes('ADR-0089')).length, 1);
  assert.equal(files.some((relative) => relative.startsWith('forge-core/')), false);
  assert.equal(files.some((relative) => relative.startsWith('forge-runtime/')), false);
});

test('fresh exact18 generated project passes 33/skip2 and governance 12/skip2', (t) => {
  const target = initializedProject(t, 'kernel-operational-fresh-',
    'kernel-operational-fresh');
  assert.doesNotThrow(() => assertKernelOperationalReferenceScaffold(target));
});

test('bytes, mode, links, package, detector, ledger and parity residue fail closed', (t) => {
  const target = initializedProject(t, 'kernel-operational-negative-',
    'kernel-operational-negative');
  assert.doesNotThrow(() => assertKernelOperationalReferenceProjection(target));
  const path = join(target, MUTATION_TARGET);
  const original = readFileSync(path);
  writeFileSync(path, Buffer.concat([original, Buffer.from('\n# drift\n')]));
  assert.throws(() => assertKernelOperationalReferenceProjection(target),
    /byte-identical to source/);
  restoreExact(target, MUTATION_TARGET);
  chmodSync(path, 0o600);
  assert.throws(() => assertKernelOperationalReferenceProjection(target), /0644/);
  chmodSync(path, 0o644);
  const alias = join(target, 'kernel-operational-hardlink.py');
  copyFileSync(path, alias);
  rmSync(path);
  linkSync(alias, path);
  assert.throws(() => assertKernelOperationalReferenceProjection(target), /hardlink/);
  rmSync(path);
  rmSync(alias);
  symlinkSync(join(SOURCE_ROOT, MUTATION_TARGET), path);
  assert.throws(() => assertKernelOperationalReferenceProjection(target), /symlink/);
  restoreExact(target, MUTATION_TARGET);

  const extra = join(target, PACKAGE, 'extra.py');
  writeFileSync(extra, 'unexpected\n');
  assert.throws(() => assertKernelOperationalReferenceProjection(target),
    /exact eight-file closure/);
  rmSync(extra);
  const extraDir = join(target, PACKAGE, 'cache');
  mkdirSync(extraDir);
  assert.throws(() => assertKernelOperationalReferenceProjection(target),
    /exact eight-file closure/);
  rmSync(extraDir, { recursive: true });

  const originalState = state(target).copied;
  writeState(target, originalState.filter((relative) => relative !== MUTATION_TARGET));
  assert.throws(() => assertKernelOperationalReferenceProjection(target),
    /ledger entry missing/);
  writeState(target, [...originalState,
    'harness/kernel_operational_reference_residue.py']);
  assert.throws(() => assertKernelOperationalReferenceProjection(target), /exact18/);
  writeState(target, originalState);

  const detectorPath = join(target, DETECTORS);
  const detectors = readFileSync(detectorPath, 'utf8');
  writeFileSync(detectorPath, detectors.replace(
    'argv: [python3, harness/kernel_operational_contract_check.py, --golden, .]',
    'argv: [python3, harness/kernel_operational_contract_check.py, --golden, ., repo_root]',
  ));
  assert.throws(() => assertKernelOperationalReferenceProjection(target),
    /argv/);
  restoreExact(target, DETECTORS);

  const policyPath = join(target, POLICY);
  const policy = readFileSync(policyPath, 'utf8');
  writeFileSync(policyPath, policy.replace(
    'copies_go_rust_or_runtime_registration: false',
    'copies_go_rust_or_runtime_registration: true',
  ));
  assert.throws(() => assertKernelOperationalReferenceProjection(target),
    /frozen Registry v39 pin/);
  restoreExact(target, POLICY);

  for (const relative of [
    join('forge-core', 'internal', 'kerneloperationalcontract'),
    join('forge-runtime', 'crates', 'domain', 'src', 'kernel_operational_contract'),
  ]) {
    mkdirSync(join(target, relative), { recursive: true });
    assert.throws(() => assertNoKernelOperationalReferenceInstall(target),
      /must not install/);
    rmSync(join(target, relative), { recursive: true });
  }
});

test('real Registry v36 projection upgrades through v39 and is idempotent', (t) => {
  const target = initializedProject(t, 'kernel-operational-v36-',
    'kernel-operational-v36');
  seedRegistryV36Projection(target);
  assertRegistryV36Projection(target, SOURCE_ROOT);
  const result = run({
    from: SOURCE_ROOT, target, apply: true, backup: true, prune: false,
  });
  assert.deepEqual([...result.drift.added].sort(),
    [...UPGRADE_ADDED_FILES].sort());
  assert.deepEqual([...result.drift.changed].sort(),
    [...UPGRADE_CHANGED_FILES].sort());
  for (const relative of [
    ...UPGRADE_ADDED_FILES, ...UPGRADE_CHANGED_FILES,
  ]) {
    assert.deepEqual(readFileSync(join(target, relative)),
      readFileSync(join(SOURCE_ROOT, relative)), `upgraded bytes drifted: ${relative}`);
    const targetInfo = lstatSync(join(target, relative));
    const sourceInfo = lstatSync(join(SOURCE_ROOT, relative));
    assert.equal(targetInfo.mode & 0o777, sourceInfo.mode & 0o777,
      `upgraded mode drifted: ${relative}`);
    assert.equal(targetInfo.nlink, 1, `upgraded link count drifted: ${relative}`);
  }
  assert.doesNotThrow(() => assertKernelOperationalReferenceScaffold(target));
  const second = run({
    from: SOURCE_ROOT, target, apply: true, backup: false, prune: false,
  });
  assert.deepEqual(second.drift.added, []);
  assert.deepEqual(second.drift.changed, []);
});
