import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  chmodSync, copyFileSync, linkSync, mkdirSync, mkdtempSync,
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
  assertKernelDecisionReferenceProjection,
  assertKernelDecisionReferenceScaffold,
  assertNoKernelDecisionReferenceInstall,
} from './kernel-decision-reference-upgrade-verification.mjs';
import {
  assertRegistryV37Projection, seedRegistryV37Projection,
} from './kernel-decision-reference-v37-projection.mjs';
import {
  DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES,
} from './decision-capsule-structural-replay-copy-fragment.mjs';
import {
  REGISTRY_V39_SHARED_OWNER_FILES,
} from './decision-capsule-structural-replay-v38-projection.mjs';
import {
  assertRegistryV36Projection, seedRegistryV36Projection,
} from './kernel-operational-reference-v36-projection.mjs';
import {
  assertRegistryV35Projection, seedRegistryV35Projection,
} from './legacy-governance-read-import-v35-projection.mjs';
import {
  renderScaffoldState, SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';
import { run } from './forge-upgrade.mjs';

const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const DETECTORS = join('.agent', 'engineering', 'detectors.yml');
const PACKAGE = join('harness', 'kernel_decision_contract');
const MUTATION_TARGET = join('harness', 'governance_engineering',
  'kernel_decision_reference_candidate.py');

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

test('exact19 is closed, source-only, disjoint from exact18 and excludes parity', () => {
  const files = KERNEL_DECISION_REFERENCE_EXPECTED_FILES;
  assert.equal(files.length, 19);
  assert.equal(new Set(files).size, 19);
  assert.equal(files.filter((relative) => relative.includes('ADR-0090')).length, 1);
  assert.equal(files.filter((relative) => relative.includes('ADR-0091')).length, 1);
  assert.deepEqual(files.filter((relative) =>
    KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES.includes(relative)), []);
  assert.equal(files.some((relative) => relative.startsWith('forge-core/')), false);
  assert.equal(files.some((relative) => relative.startsWith('forge-runtime/')), false);
});

test('fresh exact19 generated project passes core and governance evidence', (t) => {
  const target = initializedProject(t, 'kernel-decision-fresh-', 'kernel-decision-fresh');
  assert.doesNotThrow(() => assertKernelDecisionReferenceScaffold(target));
});

test('bytes, mode, links, package, detector, ledger and parity residue fail closed', (t) => {
  const target = initializedProject(t, 'kernel-decision-negative-',
    'kernel-decision-negative');
  assert.doesNotThrow(() => assertKernelDecisionReferenceProjection(target));
  const path = join(target, MUTATION_TARGET);
  const original = readFileSync(path);
  writeFileSync(path, Buffer.concat([original, Buffer.from('\n# drift\n')]));
  assert.throws(() => assertKernelDecisionReferenceProjection(target),
    /byte-identical to source/);
  restoreExact(target, MUTATION_TARGET);
  chmodSync(path, 0o600);
  assert.throws(() => assertKernelDecisionReferenceProjection(target), /0644/);
  chmodSync(path, 0o644);
  const alias = join(target, 'kernel-decision-hardlink.py');
  copyFileSync(path, alias);
  rmSync(path);
  linkSync(alias, path);
  assert.throws(() => assertKernelDecisionReferenceProjection(target), /hardlink/);
  rmSync(path);
  rmSync(alias);
  symlinkSync(join(SOURCE_ROOT, MUTATION_TARGET), path);
  assert.throws(() => assertKernelDecisionReferenceProjection(target), /symlink/);
  restoreExact(target, MUTATION_TARGET);

  const extra = join(target, PACKAGE, 'extra.py');
  writeFileSync(extra, 'unexpected\n');
  assert.throws(() => assertKernelDecisionReferenceProjection(target),
    /exact nine-file closure/);
  rmSync(extra);
  const extraDir = join(target, PACKAGE, 'cache');
  mkdirSync(extraDir);
  assert.throws(() => assertKernelDecisionReferenceProjection(target),
    /exact nine-file closure/);
  rmSync(extraDir, { recursive: true });

  const originalState = state(target).copied;
  writeState(target, originalState.filter((relative) => relative !== MUTATION_TARGET));
  assert.throws(() => assertKernelDecisionReferenceProjection(target),
    /ledger entry missing/);
  writeState(target, [...originalState,
    'harness/kernel_decision_reference_residue.py']);
  assert.throws(() => assertKernelDecisionReferenceProjection(target), /exact19/);
  writeState(target, originalState);

  const detectorPath = join(target, DETECTORS);
  const detectors = readFileSync(detectorPath, 'utf8');
  writeFileSync(detectorPath, detectors.replace(
    'argv: [python3, harness/kernel_decision_contract_check.py, --golden, .]',
    'argv: [python3, harness/kernel_decision_contract_check.py, --golden, ., repo_root]',
  ));
  assert.throws(() => assertKernelDecisionReferenceProjection(target), /argv/);
  restoreExact(target, DETECTORS);

  const policyPath = join(target, POLICY);
  writeFileSync(policyPath, Buffer.concat([
    readFileSync(policyPath), Buffer.from('\n# unpinned drift\n'),
  ]));
  assert.throws(() => assertKernelDecisionReferenceProjection(target),
    /frozen Registry v39 pin/);
  restoreExact(target, POLICY);

  for (const relative of [
    join('forge-core', 'internal', 'kerneldecisioncontract'),
    join('forge-runtime', 'crates', 'domain', 'src', 'kernel_decision_contract'),
  ]) {
    mkdirSync(join(target, relative), { recursive: true });
    assert.throws(() => assertNoKernelDecisionReferenceInstall(target),
      /must not install/);
    rmSync(join(target, relative), { recursive: true });
  }
});

test('real Registry v37 projection upgrades through v39 and is idempotent', (t) => {
  const target = initializedProject(t, 'kernel-decision-v37-', 'kernel-decision-v37');
  seedRegistryV37Projection(target);
  assertRegistryV37Projection(target, SOURCE_ROOT);
  const result = run({
    from: SOURCE_ROOT, target, apply: true, backup: true, prune: false,
  });
  const added = [
    ...KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
    ...DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES,
  ];
  const changed = REGISTRY_V39_SHARED_OWNER_FILES.filter(
    (relative) => !added.includes(relative));
  assert.deepEqual([...result.drift.added].sort(), [...added].sort());
  assert.deepEqual([...result.drift.changed].sort(), [...changed].sort());
  for (const relative of [
    ...added, ...changed,
  ]) {
    assert.deepEqual(readFileSync(join(target, relative)),
      readFileSync(join(SOURCE_ROOT, relative)), `upgraded bytes drifted: ${relative}`);
  }
  assert.doesNotThrow(() => assertKernelDecisionReferenceScaffold(target));
  const second = run({
    from: SOURCE_ROOT, target, apply: true, backup: false, prune: false,
  });
  assert.deepEqual(second.drift.added, []);
  assert.deepEqual(second.drift.changed, []);
});

test('v39 inverse composes through frozen v36 and v35 verifiers', (t) => {
  const v36 = initializedProject(t, 'kernel-decision-v36-', 'kernel-decision-v36');
  seedRegistryV36Projection(v36);
  assertRegistryV36Projection(v36, SOURCE_ROOT);
  const v35 = initializedProject(t, 'kernel-decision-v35-', 'kernel-decision-v35');
  seedRegistryV35Projection(v35);
  assertRegistryV35Projection(v35, SOURCE_ROOT);
});
