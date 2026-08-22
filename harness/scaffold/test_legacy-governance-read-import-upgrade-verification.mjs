import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  chmodSync, closeSync, constants, copyFileSync, fstatSync, linkSync, lstatSync,
  mkdirSync, mkdtempSync, openSync, readFileSync, rmSync, symlinkSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES,
} from './legacy-governance-read-import-copy-fragment.mjs';
import {
  KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
} from './kernel-decision-reference-copy-fragment.mjs';
import {
  KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES,
} from './kernel-operational-reference-copy-fragment.mjs';
import {
  assertLegacyGovernanceReadImportProjection,
  assertLegacyGovernanceReadImportScaffold,
  assertNoLegacyGovernanceReadImportInstall,
} from './legacy-governance-read-import-upgrade-verification.mjs';
import {
  assertKernelOperationalReferenceProjection,
} from './kernel-operational-reference-upgrade-verification.mjs';
import {
  assertRegistryV35Projection,
  CONCURRENT_CURRENT_FILES,
  REGISTRY_V36_OWNER_FILES,
  seedRegistryV35Projection,
} from './legacy-governance-read-import-v35-projection.mjs';
import {
  REGISTRY_V37_OWNER_FILES,
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
const REQUEST = join('docs', 'contracts', 'fixtures',
  'legacy-governance-read-import-request-v1.json');
const VIEW = join('docs', 'contracts', 'fixtures',
  'legacy-governance-read-import-view-v1.json');
const CHECKER = join('harness', 'legacy_governance_read_import_contract_check.py');
const PACKAGE = join('harness', 'legacy_governance_read_import_contract');
const MUTATION_TARGET = join('harness', 'governance_engineering',
  'legacy_governance_read_import_candidate.py');
const SUCCESS_MARKER = 'STRUCTURALLY_VALID_LEGACY_GOVERNANCE_READ_IMPORT_V1';
const V35_UPGRADE_ADDED = [
  ...LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES,
  ...KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES,
  ...KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
  ...DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES,
];
const V35_UPGRADE_CHANGED = [...new Set([
  ...REGISTRY_V36_OWNER_FILES, ...CONCURRENT_CURRENT_FILES,
  ...REGISTRY_V37_OWNER_FILES, ...REGISTRY_V39_SHARED_OWNER_FILES,
])].filter((relative) => !V35_UPGRADE_ADDED.includes(relative));

function initialize(target, name) {
  return spawnSync(process.execPath, [
    'harness/scaffold/forge-init.mjs', target, '--name', name,
  ], { cwd: SOURCE_ROOT, encoding: 'utf8' });
}

function initializedProject(t, prefix, name) {
  const root = mkdtempSync(join(tmpdir(), prefix));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const result = initialize(target, name);
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

function assertExactCheckerBytes(target) {
  const expected = readFileSync(join(target, VIEW));
  const path = join(target, REQUEST);
  const before = lstatSync(path);
  const fd = openSync(path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));
  try {
    const opened = fstatSync(fd);
    assert.equal(opened.isFile(), true);
    assert.equal(opened.nlink, 1);
    assert.equal(opened.dev, before.dev);
    assert.equal(opened.ino, before.ino);
    const result = spawnSync('python3', ['-S', '-B', CHECKER], {
      cwd: target,
      stdio: [fd, 'pipe', 'pipe'],
      timeout: 5000,
      env: { PATH: process.env.PATH, PYTHONDONTWRITEBYTECODE: '1' },
    });
    assert.equal(result.error, undefined, result.error?.message);
    assert.equal(result.signal, null);
    assert.equal(result.status, 0, result.stderr.toString());
    assert.deepEqual(result.stdout, expected,
      'checker must emit only exact canonical view bytes plus LF');
    assert.equal(result.stderr.length, 0);
    assert.equal(result.stdout.includes(Buffer.from(SUCCESS_MARKER)), false,
      'checker must never emit the governance success marker');
  } finally {
    closeSync(fd);
  }
}

test('exact18 is closed, source-only and excludes Catalyst Go parity', () => {
  const files = LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES;
  assert.equal(files.length, 18);
  assert.equal(new Set(files).size, 18);
  assert.equal(files.some((relative) => relative.startsWith('forge-core/')), false);
  assert.equal(files.filter((relative) => relative.includes('ADR-0086')).length, 1);
  assert.equal(files.filter((relative) => relative.includes('ADR-0087')).length, 1);
});

test('fresh exact18 generated project passes and checker stdout is exact view', (t) => {
  const target = initializedProject(t, 'legacy-import-fresh-', 'legacy-import-fresh');
  assert.doesNotThrow(() => assertLegacyGovernanceReadImportScaffold(target));
  assert.doesNotThrow(() => assertKernelOperationalReferenceProjection(target));
  assertExactCheckerBytes(target);
});

test('bytes, mode, links, package, detector, ledger and Go residue fail closed', (t) => {
  const target = initializedProject(t, 'legacy-import-negative-', 'legacy-import-negative');
  assert.doesNotThrow(() => assertLegacyGovernanceReadImportProjection(target));
  const path = join(target, MUTATION_TARGET);
  const original = readFileSync(path);
  writeFileSync(path, Buffer.concat([original, Buffer.from('\n# drift\n')]));
  assert.throws(() => assertLegacyGovernanceReadImportProjection(target),
    /byte-identical to source/);
  restoreExact(target, MUTATION_TARGET);
  chmodSync(path, 0o600);
  assert.throws(() => assertLegacyGovernanceReadImportProjection(target), /mode 0644/);
  chmodSync(path, 0o644);
  const alias = join(target, 'legacy-import-hardlink.py');
  copyFileSync(path, alias);
  rmSync(path);
  linkSync(alias, path);
  assert.throws(() => assertLegacyGovernanceReadImportProjection(target), /hardlink/);
  rmSync(path);
  rmSync(alias);
  symlinkSync(join(SOURCE_ROOT, MUTATION_TARGET), path);
  assert.throws(() => assertLegacyGovernanceReadImportProjection(target), /symlink/);
  restoreExact(target, MUTATION_TARGET);
  rmSync(path);
  const fifo = spawnSync('mkfifo', [path]);
  if (fifo.status === 0) {
    assert.throws(() => assertLegacyGovernanceReadImportProjection(target),
      /non-file|unsafe/);
    rmSync(path);
    restoreExact(target, MUTATION_TARGET);
  }

  const packagePath = join(target, PACKAGE);
  for (const name of ['extra.py', 'extra.txt']) {
    const extra = join(packagePath, name);
    writeFileSync(extra, 'unexpected\n');
    assert.throws(() => assertLegacyGovernanceReadImportProjection(target),
      /exact six-file closure/);
    rmSync(extra);
  }
  rmSync(packagePath, { recursive: true });
  symlinkSync(join(SOURCE_ROOT, PACKAGE), packagePath, 'dir');
  assert.throws(() => assertLegacyGovernanceReadImportProjection(target), /symlink/);
  rmSync(packagePath);
  mkdirSync(packagePath);
  for (const relative of LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES
    .filter((item) => item.startsWith(`${PACKAGE}/`))) {
    copyFileSync(join(SOURCE_ROOT, relative), join(target, relative));
    chmodSync(join(target, relative), 0o644);
  }

  const originalState = state(target).copied;
  writeState(target, originalState.filter((relative) => relative !== MUTATION_TARGET));
  assert.throws(() => assertLegacyGovernanceReadImportProjection(target), /ledger entry missing/);
  writeState(target, [...originalState, 'harness/legacy_governance_read_import_residue.py']);
  assert.throws(() => assertLegacyGovernanceReadImportProjection(target), /exact18/);
  writeState(target, [...originalState,
    'forge-core/internal/legacygovernanceimportcontract/residue.go']);
  assert.throws(() => assertLegacyGovernanceReadImportProjection(target), /exact18/);
  writeState(target, originalState);

  const detectorPath = join(target, DETECTORS);
  const detectors = readFileSync(detectorPath, 'utf8');
  writeFileSync(detectorPath, detectors.replace(
    'argv: [python3, harness/legacy_governance_read_import_contract_check.py]',
    'argv: [python3, harness/legacy_governance_read_import_contract_check.py, repo_root]',
  ));
  assert.throws(() => assertLegacyGovernanceReadImportProjection(target),
    /detector argv/);
  restoreExact(target, DETECTORS);

  const policyPath = join(target, POLICY);
  const policy = readFileSync(policyPath, 'utf8');
  writeFileSync(policyPath, policy.replace(
    'copies_go_parity_package_or_runtime: false',
    'copies_go_parity_package_or_runtime: true',
  ));
  assert.throws(() => assertLegacyGovernanceReadImportProjection(target),
    /frozen Registry v39 pin/);
  restoreExact(target, POLICY);

  const go = join(target, 'forge-core', 'internal', 'legacygovernanceimportcontract');
  mkdirSync(go, { recursive: true });
  assert.throws(() => assertNoLegacyGovernanceReadImportInstall(target),
    /must not install/);
});

test('real Registry v35 projection upgrades through v39 and is byte-idempotent', (t) => {
  const target = initializedProject(t, 'legacy-import-v35-', 'legacy-import-v35');
  seedRegistryV35Projection(target);
  assertRegistryV35Projection(target, SOURCE_ROOT);
  const result = run({
    from: SOURCE_ROOT, target, apply: true, backup: true, prune: false,
  });
  assert.deepEqual([...result.drift.added].sort(),
    [...V35_UPGRADE_ADDED].sort());
  assert.deepEqual([...result.drift.changed].sort(),
    [...V35_UPGRADE_CHANGED].sort());
  for (const relative of V35_UPGRADE_CHANGED) {
    assert.deepEqual(readFileSync(join(target, relative)),
      readFileSync(join(SOURCE_ROOT, relative)), `owner bytes drifted: ${relative}`);
    const targetInfo = lstatSync(join(target, relative));
    const sourceInfo = lstatSync(join(SOURCE_ROOT, relative));
    assert.equal(targetInfo.mode & 0o777, sourceInfo.mode & 0o777,
      `owner mode drifted: ${relative}`);
    assert.equal(targetInfo.nlink, 1, `owner link count drifted: ${relative}`);
  }
  for (const relative of V35_UPGRADE_ADDED) {
    assert.deepEqual(readFileSync(join(target, relative)),
      readFileSync(join(SOURCE_ROOT, relative)), `upgraded bytes drifted: ${relative}`);
    const info = lstatSync(join(target, relative));
    assert.equal(info.mode & 0o777, 0o644, `upgraded mode drifted: ${relative}`);
    assert.equal(info.nlink, 1, `upgraded link count drifted: ${relative}`);
  }
  assert.doesNotThrow(() => assertLegacyGovernanceReadImportScaffold(target));
  const second = run({
    from: SOURCE_ROOT, target, apply: true, backup: false, prune: false,
  });
  assert.deepEqual(second.drift.added, []);
  assert.deepEqual(second.drift.changed, []);
});
