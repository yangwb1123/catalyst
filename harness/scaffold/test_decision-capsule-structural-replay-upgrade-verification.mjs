import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  chmodSync, copyFileSync, existsSync, linkSync, mkdirSync, mkdtempSync,
  readFileSync, renameSync, rmSync, symlinkSync, unlinkSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES,
} from './decision-capsule-structural-replay-copy-fragment.mjs';
import {
  KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
} from './kernel-decision-reference-copy-fragment.mjs';
import {
  assertDecisionCapsuleStructuralReplayProjection,
  assertDecisionCapsuleStructuralReplayScaffold,
  assertNoDecisionCapsuleStructuralReplayInstall,
} from './decision-capsule-structural-replay-upgrade-verification.mjs';
import {
  assertRegistryV38Projection, REGISTRY_V39_SHARED_OWNER_FILES,
  seedRegistryV38Projection,
} from './decision-capsule-structural-replay-v38-projection.mjs';
import {
  assertRegistryV37Projection, seedRegistryV37Projection,
} from './kernel-decision-reference-v37-projection.mjs';
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
const PACKAGE = join('harness', 'decision_capsule_contract');
const MUTATION_TARGET = join('harness', 'governance_engineering',
  'decision_capsule_structural_replay_candidate.py');

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

test('Decision Capsule exact19 is closed, source-only and disjoint from prior exact19', () => {
  const files = DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES;
  assert.equal(files.length, 19);
  assert.equal(new Set(files).size, 19);
  assert.equal(files.filter((relative) => relative.includes('ADR-0092')).length, 1);
  assert.equal(files.filter((relative) => relative.includes('ADR-0093')).length, 1);
  assert.deepEqual(files.filter((relative) =>
    KERNEL_DECISION_REFERENCE_EXPECTED_FILES.includes(relative)), []);
  assert.equal(files.some((relative) => relative.startsWith('forge-core/')), false);
  assert.equal(files.some((relative) => relative.startsWith('forge-runtime/')), false);
});

test('fresh Decision Capsule exact19 passes core and governance evidence', (t) => {
  const target = initializedProject(
    t, 'decision-capsule-fresh-', 'decision-capsule-fresh');
  assert.doesNotThrow(() =>
    assertDecisionCapsuleStructuralReplayScaffold(target));
});

test('bytes, mode, links, package, detector, ledger and parity residue fail closed', (t) => {
  const target = initializedProject(
    t, 'decision-capsule-negative-', 'decision-capsule-negative');
  assert.doesNotThrow(() =>
    assertDecisionCapsuleStructuralReplayProjection(target));
  const path = join(target, MUTATION_TARGET);
  const original = readFileSync(path);
  writeFileSync(path, Buffer.concat([original, Buffer.from('\n# drift\n')]));
  assert.throws(() => assertDecisionCapsuleStructuralReplayProjection(target),
    /byte-identical to source/);
  restoreExact(target, MUTATION_TARGET);
  chmodSync(path, 0o600);
  assert.throws(() => assertDecisionCapsuleStructuralReplayProjection(target),
    /0644/);
  chmodSync(path, 0o644);
  const alias = join(target, 'decision-capsule-hardlink.py');
  copyFileSync(path, alias);
  rmSync(path);
  linkSync(alias, path);
  assert.throws(() => assertDecisionCapsuleStructuralReplayProjection(target),
    /hardlink/);
  rmSync(path);
  rmSync(alias);
  symlinkSync(join(SOURCE_ROOT, MUTATION_TARGET), path);
  assert.throws(() => assertDecisionCapsuleStructuralReplayProjection(target),
    /symlink/);
  restoreExact(target, MUTATION_TARGET);

  const extra = join(target, PACKAGE, 'extra.py');
  writeFileSync(extra, 'unexpected\n');
  assert.throws(() => assertDecisionCapsuleStructuralReplayProjection(target),
    /exact nine-file closure/);
  rmSync(extra);
  const extraDir = join(target, PACKAGE, 'cache');
  mkdirSync(extraDir);
  assert.throws(() => assertDecisionCapsuleStructuralReplayProjection(target),
    /exact nine-file closure/);
  rmSync(extraDir, { recursive: true });

  const originalState = state(target).copied;
  writeState(target, originalState.filter(
    (relative) => relative !== MUTATION_TARGET));
  assert.throws(() => assertDecisionCapsuleStructuralReplayProjection(target),
    /ledger entry missing/);
  writeState(target, [...originalState,
    'harness/decision_capsule_structural_replay_residue.py']);
  assert.throws(() => assertDecisionCapsuleStructuralReplayProjection(target),
    /exact19/);
  writeState(target, originalState);

  const detectorPath = join(target, DETECTORS);
  const detectors = readFileSync(detectorPath, 'utf8');
  writeFileSync(detectorPath, detectors.replace(
    'argv: [python3, harness/decision_capsule_contract_check.py, --golden, .]',
    'argv: [python3, harness/decision_capsule_contract_check.py, --golden, ., repo_root]',
  ));
  assert.throws(() => assertDecisionCapsuleStructuralReplayProjection(target),
    /argv/);
  restoreExact(target, DETECTORS);

  const policyPath = join(target, POLICY);
  writeFileSync(policyPath, Buffer.concat([
    readFileSync(policyPath), Buffer.from('\n# unpinned drift\n'),
  ]));
  assert.throws(() => assertDecisionCapsuleStructuralReplayProjection(target),
    /frozen Registry v39 pin/);
  restoreExact(target, POLICY);

  for (const relative of [
    join('forge-core', 'internal', 'decisioncapsulecontract'),
    join('forge-runtime', 'crates', 'domain', 'src',
      'decision_capsule_contract'),
  ]) {
    mkdirSync(join(target, relative), { recursive: true });
    assert.throws(() => assertNoDecisionCapsuleStructuralReplayInstall(target),
      /must not install/);
    rmSync(join(target, relative), { recursive: true });
  }
});

test('real Registry v38 projection upgrades exact19 owners and is idempotent', (t) => {
  const target = initializedProject(
    t, 'decision-capsule-v38-', 'decision-capsule-v38');
  seedRegistryV38Projection(target);
  assertRegistryV38Projection(target, SOURCE_ROOT);
  const v38Check = spawnSync(
    'python3', ['-B', 'harness/governance_engineering_check.py'],
    { cwd: target, encoding: 'utf8', env: { ...process.env, PYTHONDONTWRITEBYTECODE: '1' } },
  );
  assert.equal(v38Check.status, 0,
    `frozen v38 governance checker failed:\n${v38Check.stdout}\n${v38Check.stderr}`);
  const result = run({
    from: SOURCE_ROOT, target, apply: true, backup: true, prune: false,
  });
  assert.deepEqual([...result.drift.added].sort(),
    [...DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES].sort());
  assert.deepEqual([...result.drift.changed].sort(),
    [...REGISTRY_V39_SHARED_OWNER_FILES].sort());
  for (const relative of [
    ...DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES,
    ...REGISTRY_V39_SHARED_OWNER_FILES,
  ]) assert.deepEqual(readFileSync(join(target, relative)),
    readFileSync(join(SOURCE_ROOT, relative)),
  `upgraded bytes drifted: ${relative}`);
  assert.doesNotThrow(() =>
    assertDecisionCapsuleStructuralReplayScaffold(target));
  const second = run({
    from: SOURCE_ROOT, target, apply: true, backup: false, prune: false,
  });
  assert.deepEqual(second.drift.added, []);
  assert.deepEqual(second.drift.changed, []);
});

test('v38 upgrade rejects an unowned exact19 collision before every write', (t) => {
  const target = initializedProject(
    t, 'decision-capsule-v38-collision-', 'decision-capsule-v38-collision');
  seedRegistryV38Projection(target);
  assertRegistryV38Projection(target, SOURCE_ROOT);
  const collision = DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES[0];
  const collisionBytes = Buffer.from('user-owned v38 path\n');
  mkdirSync(dirname(join(target, collision)), { recursive: true });
  writeFileSync(join(target, collision), collisionBytes);
  const policyBefore = readFileSync(join(target, POLICY));
  assert.throws(
    () => run({
      from: SOURCE_ROOT, target, apply: true, backup: false, prune: false,
    }),
    /refusing to overwrite unowned scaffold path/,
  );
  assert.deepEqual(readFileSync(join(target, collision)), collisionBytes);
  assert.deepEqual(readFileSync(join(target, POLICY)), policyBefore);
  assert.equal(state(target).copied.includes(collision), false);
});

test('v38 upgrade atomically rejects a post-classification exact19 collision', (t) => {
  const target = initializedProject(
    t, 'decision-capsule-v38-race-', 'decision-capsule-v38-race');
  seedRegistryV38Projection(target);
  assertRegistryV38Projection(target, SOURCE_ROOT);
  const ordered = [...DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES].sort();
  const collision = ordered.at(-1);
  const collisionBytes = Buffer.from('concurrent user-owned v38 path\n');
  const policyBefore = readFileSync(join(target, POLICY));
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const stateBefore = readFileSync(statePath);
  assert.throws(
    () => run(
      { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false },
      new Date('2026-08-20T00:00:00.000Z'),
      { afterClassification({ drift }) {
        assert.equal(drift.added.includes(collision), true);
        mkdirSync(dirname(join(target, collision)), { recursive: true });
        writeFileSync(join(target, collision), collisionBytes);
      } },
    ),
    /refusing to overwrite unowned scaffold path/,
  );
  assert.deepEqual(readFileSync(join(target, collision)), collisionBytes);
  for (const relative of ordered.slice(0, -1)) {
    assert.equal(existsSync(join(target, relative)), false,
      `rolled-back reservation remained: ${relative}`);
  }
  assert.equal(existsSync(join(target, PACKAGE)), false,
    'rollback must remove a transaction-created package directory');
  assert.deepEqual(readFileSync(join(target, POLICY)), policyBefore);
  assert.deepEqual(readFileSync(statePath), stateBefore);
});

test('v38 upgrade preserves a file replacing an exclusive reservation', (t) => {
  const target = initializedProject(
    t, 'decision-capsule-v38-replacement-', 'decision-capsule-v38-replacement');
  seedRegistryV38Projection(target);
  assertRegistryV38Projection(target, SOURCE_ROOT);
  const ordered = [...DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES].sort();
  const replacement = ordered[0];
  const replacementBytes = Buffer.from('replacement after exclusive create\n');
  const policyBefore = readFileSync(join(target, POLICY));
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const stateBefore = readFileSync(statePath);
  let replaced = false;
  assert.throws(
    () => run(
      { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false },
      new Date('2026-08-20T00:00:00.000Z'),
      { afterAddedReservation({ destination, rel }) {
        if (rel !== replacement || replaced) return;
        rmSync(destination);
        writeFileSync(destination, replacementBytes);
        replaced = true;
      } },
    ),
    /refusing changed file path for added target/,
  );
  assert.equal(replaced, true);
  assert.deepEqual(readFileSync(join(target, replacement)), replacementBytes);
  for (const relative of ordered.slice(1)) {
    assert.equal(existsSync(join(target, relative)), false,
      `rolled-back descriptor-owned reservation remained: ${relative}`);
  }
  assert.deepEqual(readFileSync(join(target, POLICY)), policyBefore);
  assert.deepEqual(readFileSync(statePath), stateBefore);
});

test('v38 rollback quarantines before deciding whether a path is owned', (t) => {
  const target = initializedProject(
    t, 'decision-capsule-v38-rollback-race-', 'decision-capsule-v38-rollback-race');
  seedRegistryV38Projection(target);
  assertRegistryV38Projection(target, SOURCE_ROOT);
  const ordered = [...DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES].sort();
  const replacement = ordered.at(-1);
  const replacementBytes = Buffer.from('replacement at rollback rename boundary\n');
  const policyBefore = readFileSync(join(target, POLICY));
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const stateBefore = readFileSync(statePath);
  let replaced = false;
  assert.throws(
    () => run(
      { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false },
      new Date('2026-08-20T00:00:00.000Z'),
      {
        afterAddedReservation({ rel }) {
          if (rel === replacement) throw new Error('forced reservation rollback');
        },
        beforeAddedRollbackRename({ destination, rel }) {
          if (rel !== replacement || replaced) return;
          rmSync(destination);
          writeFileSync(destination, replacementBytes);
          replaced = true;
        },
      },
    ),
    /forced reservation rollback/,
  );
  assert.equal(replaced, true);
  assert.deepEqual(readFileSync(join(target, replacement)), replacementBytes);
  for (const relative of ordered.slice(0, -1)) {
    assert.equal(existsSync(join(target, relative)), false);
  }
  assert.deepEqual(readFileSync(join(target, POLICY)), policyBefore);
  assert.deepEqual(readFileSync(statePath), stateBefore);
});

test('v38 upgrade retracts its ledger when an added path changes at commit', (t) => {
  const target = initializedProject(
    t, 'decision-capsule-v38-commit-race-', 'decision-capsule-v38-commit-race');
  seedRegistryV38Projection(target);
  assertRegistryV38Projection(target, SOURCE_ROOT);
  const ordered = [...DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES].sort();
  const replacement = ordered[0];
  const replacementBytes = Buffer.from('replacement after ledger write\n');
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const stateBefore = readFileSync(statePath);
  const ownersBefore = new Map(REGISTRY_V39_SHARED_OWNER_FILES.map(
    (relative) => [relative, readFileSync(join(target, relative))]));
  let replaced = false;
  assert.throws(
    () => run(
      { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false },
      new Date('2026-08-20T00:00:00.000Z'),
      { afterScaffoldStateWrite() {
        rmSync(join(target, replacement));
        writeFileSync(join(target, replacement), replacementBytes);
        replaced = true;
      } },
    ),
    /refusing changed file path for added target/,
  );
  assert.equal(replaced, true);
  assert.deepEqual(readFileSync(join(target, replacement)), replacementBytes);
  for (const relative of ordered.slice(1)) {
    assert.equal(existsSync(join(target, relative)), false);
  }
  assert.deepEqual(readFileSync(statePath), stateBefore);
  for (const [relative, bytes] of ownersBefore) {
    assert.deepEqual(readFileSync(join(target, relative)), bytes,
      `rollback left mixed owner bytes: ${relative}`);
  }
});

test('v38 upgrade rejects a same-byte ledger replacement after publication', (t) => {
  const target = initializedProject(
    t, 'decision-capsule-v38-ledger-race-', 'decision-capsule-v38-ledger-race');
  seedRegistryV38Projection(target);
  const ordered = [...DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES].sort();
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const stateBefore = readFileSync(statePath);
  let replaced = false;
  assert.throws(
    () => run(
      { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false },
      new Date('2026-08-20T00:00:00.000Z'),
      { afterScaffoldStateWrite({ path }) {
        rmSync(path);
        writeFileSync(path, stateBefore);
        replaced = true;
      } },
    ),
    /changed scaffold-state ledger|preserved prior scaffold-state/,
  );
  assert.equal(replaced, true);
  assert.deepEqual(readFileSync(statePath), stateBefore);
  for (const relative of ordered) assert.equal(existsSync(join(target, relative)), false);
});

test('v38 upgrade rejects and retracts a descriptor-pinned parent escape', (t) => {
  const target = initializedProject(
    t, 'decision-capsule-v38-parent-race-', 'decision-capsule-v38-parent-race');
  seedRegistryV38Projection(target);
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const stateBefore = readFileSync(statePath);
  const detached = join(dirname(target), 'detached-added-parent');
  let attacked = null;
  try {
    assert.throws(
      () => run(
        { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false },
        new Date('2026-08-20T00:00:00.000Z'),
        { afterAddedReservation({ destination, rel }) {
          if (attacked !== null) return;
          attacked = { destination, rel };
          renameSync(dirname(destination), detached);
          symlinkSync(detached, dirname(destination), 'dir');
        } },
      ),
      /changed directory boundary/,
    );
    assert.equal(existsSync(join(detached, attacked.rel.split('/').at(-1))), false);
  } finally {
    if (attacked !== null) {
      unlinkSync(dirname(attacked.destination));
      renameSync(detached, dirname(attacked.destination));
    }
  }
  assert.deepEqual(readFileSync(statePath), stateBefore);
});

test('frozen v38 owner manifest rejects an unrecognized inverse leak', (t) => {
  const target = initializedProject(
    t, 'decision-capsule-v38-leak-', 'decision-capsule-v38-leak');
  const owner = join('harness', 'governance_engineering',
    'test_work_intent_candidate.py');
  writeFileSync(join(target, owner), Buffer.concat([
    readFileSync(join(target, owner)), Buffer.from('\n# unrecognized future owner drift\n'),
  ]));
  seedRegistryV38Projection(target);
  assert.throws(() => assertRegistryV38Projection(target, SOURCE_ROOT),
    /frozen Registry v38 owner bytes drifted/);
});

test('v39 inverse composes through frozen v38, v37, v36 and v35', (t) => {
  const target = initializedProject(
    t, 'decision-capsule-history-', 'decision-capsule-history');
  seedRegistryV38Projection(target);
  assertRegistryV38Projection(target, SOURCE_ROOT);
  seedRegistryV37Projection(target);
  assertRegistryV37Projection(target, SOURCE_ROOT);
  seedRegistryV36Projection(target);
  assertRegistryV36Projection(target, SOURCE_ROOT);
  seedRegistryV35Projection(target);
  assertRegistryV35Projection(target, SOURCE_ROOT);
});
