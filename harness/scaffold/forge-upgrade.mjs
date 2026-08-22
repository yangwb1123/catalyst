#!/usr/bin/env node
// ForgeOS forge-upgrade — resync an ALREADY-scaffolded project's COPIED governance
// (the 70%) from a ForgeOS SOURCE repo. forge-init stamps a project once; over time
// the SOURCE harness/governance evolves (a hardened acceptance.mjs, a new arch
// check, a fixed scorecard-update) and the project's COPIED copy falls behind. This
// command brings that copy back to byte-identical-with-source — so a NEW mechanism
// reaches an OLD project without re-scaffolding (which would clobber its identity).
//
// ★ SCOPE — what upgrade DOES and DOES NOT fix (be honest, do not overclaim) ★
// There are TWO kinds of drift; upgrade addresses exactly ONE:
//   (A) COPIED harness/asset lag — the project's on-disk copy of acceptance.mjs /
//       arch-check.mjs / the .agent governance assets is older than SOURCE.
//       ► upgrade FIXES this: it resyncs the project's copied 70% to SOURCE bytes.
//   (B) forge-core (Go) BINARY behavior change — e.g. per-model latency/cost logic
//       in command_executor.go / cost.go. The project carries NO forge-core; it runs
//       the HOST's `forge` binary. Nothing in the project tree encodes that behavior.
//       ► upgrade CANNOT fix this. Upgrade your `forge` binary separately (rebuild /
//         reinstall it). This tool does not touch, and cannot change, binary behavior.
//
// ★ THE RED LINE — upgrade NEVER OVERWRITES project IDENTITY (the 30%) ★
// Universal governance is mechanically limited to the two shared manifests
// (GOVERNANCE_DIRS + COPIED_FILES). Three explicitly named `.arch` project
// instances may be seeded when absent, but their O_EXCL create path preserves a
// file that appears after planning and never truncates an existing instance.
// PROJECT/ROADMAP/CURRENT_SPRINT/project.yml/README/.gitignore/examples/ remain
// unreachable: there is no render*/writeGenerated identity code path. Tests prove
// generated identity is disjoint, configured instances remain byte-identical,
// and a concurrent create wins over a stale missing-instance plan.
//
// FAIL-SAFE: DRY by default — it reports the drift and writes NOTHING. Only an
// explicit --apply mutates the project, and even then it first BACKS UP every file
// it is about to overwrite (.forge/upgrade-backup/<timestamp>/<rel>, git-ignored)
// unless --no-backup. Idempotent: a second --apply against the same SOURCE finds
// everything unchanged and writes (and backs up) nothing.
//
// Usage:
//   node harness/forge-upgrade.mjs --from <forgeos-repo> --target <project> \
//        [--apply] [--no-backup] [--prune]
//   (default is DRY: print the drift report, write nothing)
import {
  lstatSync,
} from 'node:fs';
import { join, relative, resolve, sep } from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { fileURLToPath, pathToFileURL } from 'node:url';

// Shared SINGLE SOURCE OF TRUTH: the same manifests forge-init copies, imported
// (never re-transcribed) so scaffold and upgrade can never disagree on the 70%.
import {
  PROJECT_INSTANCE_FILES,
  SCAFFOLD_STATE_FILE,
  copiedProjection,
  renderScaffoldState,
} from './forge-init.mjs';
import {
  assertNoSymlinkComponents,
  assertSafeRegularFile,
  copyFileExclusiveNoFollow,
  releaseFileExclusiveClaim,
  snapshotFileNoFollow,
} from './scaffold-fs.mjs';
import {
  classifyDrift, classifyFrozenDrift, freezeSourceProjection,
} from './upgrade-classification.mjs';
import {
  removedFilesForProjection,
} from './upgrade-state.mjs';
import {
  withAddedReservations,
} from './upgrade-added-reservations.mjs';
import { recoverInterruptedUpgrade } from './upgrade-transaction-journal.mjs';
import {
  assertParentBoundary, captureParentBoundary, closeParentBoundary,
} from './upgrade-path-boundary.mjs';

// --- pure core (no writes; unit-testable) ------------------------------------

// manifestProjection(sourceRoot) -> the FLAT list of every relative path the 70%
// covers: each GOVERNANCE_DIR expanded through enumerateTree (its concrete files)
// plus the explicit COPIED_FILES. This is the complete overwrite/prune
// projection. Project instances are outside it and use a separate, create-only
// path; all other identity files never appear.
export function manifestProjection(sourceRoot) {
  return copiedProjection(sourceRoot);
}

function lexicalExists(path) {
  try {
    lstatSync(path);
    return true;
  } catch (err) {
    if (err?.code === 'ENOENT' || err?.code === 'ENOTDIR') return false;
    throw new Error(`cannot safely inspect ${path}: ${err.message}`);
  }
}

function validateHooks(hooks) {
  for (const name of [
    'afterClassification',
    'afterAddedReservation',
    'afterBackupReservation',
    'beforePriorDetach',
    'beforeTransactionControlDetach',
    'afterTransactionControlDetach',
    'beforeUpgradeStageDirectoryDetach',
    'beforeUpgradeStageFileCleanup',
    'afterUpgradeStageDirectoryDetach',
    'afterUpgradeStageDirectoryCreate',
    'afterUpgradeStageIntentWrite',
    'afterUpgradeStageNextSync',
    'afterUpgradePriorClaimSync',
    'afterPriorDetach',
    'beforeAddedRollbackRename',
    'beforeOwnedDirectoryQuarantineReservation',
    'beforeOwnedDirectoryFinalDetach',
    'afterScaffoldStateWrite',
    'afterTransactionCommit',
    'afterTransactionControlParentSync',
    'afterTransactionControlPrivateSync',
    'afterTransactionControlPublish',
    'afterTransactionDurability',
    'afterTransactionFinishMarker',
    'afterTransactionJournalWrite',
    'afterTransactionStart',
    'afterApplyReport',
    'afterSourceSnapshotRead',
    'afterTargetClassificationRead',
    'afterOwnedDirectoryQuarantine',
    'afterOwnedDirectoryMarkerUnlink',
  ]) {
    if (hooks[name] !== undefined && typeof hooks[name] !== 'function') {
      throw new Error(`${name} test hook must be a function`);
    }
  }
}

export { classifyDrift, freezeSourceProjection };

// --- I/O boundary ------------------------------------------------------------

// CLI arg parse. Returns a config object or throws on a usage error. DRY is the
// default (apply false); backups default ON (the safe default), --no-backup opts
// out; --prune is opt-in and only removes paths recorded by forge-init's state.
export function parseArgs(argv) {
  const out = { from: null, target: null, apply: false, backup: true, prune: false };
  const takesValue = { '--from': 'from', '--target': 'target' };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--apply') out.apply = true;
    else if (a === '--no-backup') out.backup = false;
    else if (a === '--prune') out.prune = true;
    else if (a in takesValue) {
      const v = argv[++i];
      if (v === undefined || v.startsWith('--')) {
        throw new Error(`flag ${a} requires a value (got ${v ?? 'nothing'})`);
      }
      out[takesValue[a]] = v;
    } else if (a.startsWith('--')) throw new Error(`unknown flag: ${a}`);
    else throw new Error(`unexpected argument: ${a}`);
  }
  if (!out.from) throw new Error('missing --from <forgeos-repo>');
  if (!out.target) throw new Error('missing --target <project>');
  return out;
}

// Report a commit only for a clean source. Dirty and untracked source bytes are
// copied exactly, but must never be attributed to HEAD alone.
function headProjectionMatches(sourceRoot, snapshots, options) {
  const format = spawnSync(
    'git', ['-C', sourceRoot, 'rev-parse', '--show-object-format'], options,
  );
  const tree = spawnSync(
    'git', ['-C', sourceRoot, 'ls-tree', '-rz', '--full-tree', 'HEAD'], options,
  );
  const algorithm = format.stdout?.trim();
  if (format.status !== 0 || tree.status !== 0 || !['sha1', 'sha256'].includes(algorithm)) {
    return false;
  }
  const entries = new Map(tree.stdout.split('\0').filter(Boolean).map((row) => {
    const [metadata, path] = row.split('\t');
    const [mode, type, oid] = metadata.split(' ');
    return [path, { mode, oid, type }];
  }));
  for (const [rel, snapshot] of snapshots) {
    const entry = entries.get(rel);
    const mode = snapshot.mode & 0o111 ? '100755' : '100644';
    const oid = createHash(algorithm)
      .update(`blob ${snapshot.bytes.length}\0`).update(snapshot.bytes).digest('hex');
    if (entry?.type !== 'blob' || entry.mode !== mode || entry.oid !== oid) return false;
  }
  return true;
}

export function sourceProvenance(sourceRoot, snapshots = null) {
  const options = { encoding: 'utf8', maxBuffer: 4 * 1024 * 1024 };
  const head = spawnSync(
    'git', ['-C', sourceRoot, 'rev-parse', '--short', 'HEAD'], options,
  );
  if (head.status !== 0) return 'working tree (Git provenance unavailable)';
  const sha = head.stdout.trim();
  const status = spawnSync(
    'git', [
      '-C', sourceRoot, 'status', '--porcelain=v1', '--untracked-files=all', '--ignored=matching',
    ], options,
  );
  if (status.status !== 0) {
    return `working tree based on HEAD ${sha} (dirty state unavailable)`;
  }
  const projectionClean = snapshots === null
    || headProjectionMatches(sourceRoot, snapshots, options);
  return status.stdout.length === 0 && projectionClean
    ? `clean commit ${sha}`
    : `dirty working tree based on HEAD ${sha}; copying exact working-tree bytes`;
}

// A path is retired only when the project's persisted prior projection names it,
// the current source projection no longer does, and it still exists in target.
// User-added files absent from the ledger are therefore never prune candidates.
export function removedFiles(sourceRoot, targetDir) {
  return removedFilesForProjection(manifestProjection(sourceRoot), targetDir);
}

// backupTimestamp(): a filesystem-safe ISO-ish stamp for the backup dir name
// (colons -> dashes), e.g. 2026-06-21T12-30-00-000Z. Passed in at the boundary.
function backupTimestamp(now) {
  return now.toISOString().replace(/:/g, '-');
}

// Validate the complete backup write set before the first governed overwrite.
// A bad later leaf must not allow earlier files in the batch to be mutated.
function preflightBackupLeaves(rels, backupDir) {
  if (!backupDir) return;
  for (const rel of rels) {
    const bak = join(backupDir, rel);
    assertNoSymlinkComponents(bak, `backup ${rel}`);
    if (lexicalExists(bak)) assertSafeRegularFile(bak, `backup ${rel}`);
  }
}

// printReport: the drift summary + per-file lines. THE WHOLE output in dry mode,
// the preamble in apply mode, so the operator always sees exactly what will change.
function printReport(drift, removed, projectInstances, { provenance, apply }) {
  console.log(`forge-upgrade: ${apply ? 'APPLY' : 'DRY'} — source: ${provenance}`);
  console.log(
    `  changed: ${drift.changed.length}  added: ${drift.added.length}  ` +
      `unchanged: ${drift.unchanged.length}  unowned: ${drift.unowned.length}  ` +
      `removed: ${removed.length}`,
  );
  for (const rel of drift.changed) console.log(`  changed: ${rel}`);
  for (const rel of drift.added) console.log(`  added: ${rel}`);
  for (const rel of drift.unowned) console.log(`  unowned conflict: ${rel}`);
  for (const rel of projectInstances) console.log(`  initialize project instance if missing: ${rel}`);
  for (const rel of removed) console.log(`  removed: ${rel} (in project, gone from source)`);
}

function missingProjectInstances(targetDir) {
  return PROJECT_INSTANCE_FILES.filter((rel) => !lexicalExists(join(targetDir, rel)));
}

export function seedProjectInstances(paths, sourceRoot, targetDir) {
  let initialized = 0;
  for (const rel of paths) {
    const claim = copyFileExclusiveNoFollow(
      join(sourceRoot, rel), join(targetDir, rel),
      `source ${rel}`, `project instance ${rel}`,
    );
    if (claim !== null) {
      releaseFileExclusiveClaim(claim, `project instance ${rel}`);
      initialized += 1;
    }
  }
  return initialized;
}

// printHonestScope: the standing disclaimer — upgrade resyncs the copied 70%; it
// does NOT change forge/forge-kernel runtime behavior or install authority material.
// Printed every run so the operator is never misled about the trust boundary.
function printHonestScope() {
  console.log('');
  console.log('scope: I resync this project\'s COPIED harness + governance (the 70%).');
  console.log('       Missing project-instance contracts are seeded once; existing instances are preserved.');
  console.log('       I do NOT install or upgrade `forge` / `forge-kernel` binaries.');
  console.log('       I never install trust roots, private keys, or runtime state.');
  console.log('       Upgrade host runtimes separately; a missing compatible kernel is not_executed.');
}

function printApplySummary({ written, projectInitialized, pruned, backedUp, backupDir, stateUpdated }) {
  console.log('');
  if (written === 0 && projectInitialized === 0 && pruned === 0) {
    console.log(stateUpdated
      ? `forge-upgrade: APPLIED — governance already in sync; updated ${SCAFFOLD_STATE_FILE}.`
      : 'forge-upgrade: APPLIED — already in sync; nothing written, nothing backed up.');
  } else {
    console.log(
      `forge-upgrade: APPLIED — ${written} file(s) resynced` +
        (projectInitialized > 0 ? `; ${projectInitialized} project instance file(s) initialized` : '') +
        (pruned > 0 ? `; ${pruned} retired file(s) pruned` : '') +
        (backedUp > 0 ? `; ${backedUp} changed/pruned file(s) backed up to ${backupDir}` : ''),
    );
  }
}

function preparedBackupDirectory(cfg, now, targetDir) {
  if (!cfg.backup) return null;
  const backupDir = join(targetDir, '.forge', 'upgrade-backup', backupTimestamp(now));
  assertNoSymlinkComponents(join(targetDir, '.forge'), 'backup .forge directory');
  assertNoSymlinkComponents(
    join(targetDir, '.forge', 'upgrade-backup'), 'backup root directory',
  );
  assertNoSymlinkComponents(backupDir, 'backup timestamp directory');
  return backupDir;
}

function applyUpgrade(
  cfg, now, sourceRoot, sourcePlan, targetDir, drift, removed, projectInstances,
  hooks, expectedRoot, targetSnapshots,
) {
  const backupDir = preparedBackupDirectory(cfg, now, targetDir);
  if (cfg.prune) {
    for (const rel of removed) assertSafeRegularFile(join(targetDir, rel), `target ${rel}`);
  }
  preflightBackupLeaves(
    [...drift.changed, ...(cfg.prune ? removed : [])],
    backupDir,
  );
  const current = sourcePlan.projection;
  const statePaths = cfg.prune ? current : [...new Set([...current, ...removed])];
  const backupSources = backupDir
    ? [...drift.changed, ...(cfg.prune ? removed : [])] : [];
  const outcome = withAddedReservations({
    added: drift.added,
    backupRoot: backupDir === null
      ? null : relative(targetDir, backupDir).split(sep).join('/'),
    backups: backupSources,
    changed: drift.changed,
    hookPayload: { drift, sourceRoot, targetDir },
    observed: [...drift.unchanged, ...(cfg.prune ? [] : removed)],
    projectInstances,
    removed: cfg.prune ? removed : [],
    sourceSnapshots: sourcePlan.files,
    stateContent: renderScaffoldState(statePaths),
    statePath: join(targetDir, SCAFFOLD_STATE_FILE),
    targetDir,
    targetSnapshots,
    expectedRoot,
  }, hooks);
  Object.assign(outcome, {
    backupDir,
    pruned: cfg.prune ? removed.length : 0,
    written: drift.added.length + drift.changed.length,
  });
  printApplySummary(outcome);
  printHonestScope();
  return { drift, removed, ...outcome, applied: true };
}

function preflightTargetPaths(targetDir, sourcePlan) {
  assertNoSymlinkComponents(join(targetDir, SCAFFOLD_STATE_FILE), SCAFFOLD_STATE_FILE);
  for (const rel of sourcePlan.projection) {
    assertNoSymlinkComponents(join(targetDir, rel), rel);
  }
  for (const rel of PROJECT_INSTANCE_FILES) {
    const destination = join(targetDir, rel);
    assertNoSymlinkComponents(destination, rel);
    if (lexicalExists(destination)) {
      assertSafeRegularFile(destination, `project instance ${rel}`);
    }
  }
}

function finishDryRun(drift, removed, projectInstances, hooks, sourceRoot, targetDir) {
  hooks.afterClassification?.({ drift, sourceRoot, targetDir });
  console.log('');
  console.log('forge-upgrade: DRY run — nothing written. Re-run with --apply to resync.');
  printHonestScope();
  return {
    drift, removed, projectInstances, written: 0, projectInitialized: 0,
    pruned: 0, backedUp: 0, applied: false,
  };
}

// run(cfg, now): the orchestration. Classify drift, print the report + honest
// scope, and — only on --apply — back up + write changed/added. Returns a result
// object (for tests + the exit path). Pure of process.exit so it stays testable.
function runPinned(cfg, now, hooks, sourceRoot, targetDir, session) {
  const root = session.components[0]; const expectedRoot = Object.freeze({ dev: root.dev, ino: root.ino });
  const targetView = join(session.descriptorRoot, String(root.fd));
  const sourcePlan = freezeSourceProjection(
    sourceRoot, null, hooks.afterSourceSnapshotRead,
  );
  preflightTargetPaths(targetDir, sourcePlan);
  assertParentBoundary(session);
  const recovery = cfg.apply
    ? recoverInterruptedUpgrade(targetDir, hooks, expectedRoot) : null;
  if (recovery !== null) {
    console.log(`forge-upgrade: recovered interrupted ${recovery.phase} transaction${
      recovery.transactionId ? ` ${recovery.transactionId}` : ''}`);
  }
  const classified = classifyFrozenDrift(
    sourcePlan, targetView, hooks.afterTargetClassificationRead,
  );
  const { drift, snapshots: targetSnapshots } = classified;
  const removed = removedFilesForProjection(sourcePlan.projection, targetView);
  for (const rel of removed) {
    targetSnapshots.set(
      rel, snapshotFileNoFollow(join(targetView, rel), `retired target ${rel}`),
    );
  }
  const projectInstances = missingProjectInstances(targetView);
  assertParentBoundary(session);
  for (const rel of removed) assertNoSymlinkComponents(join(targetDir, rel), rel);
  printReport(drift, removed, projectInstances, {
    provenance: sourceProvenance(sourceRoot, sourcePlan.files), apply: cfg.apply,
  });
  hooks.afterApplyReport?.({ drift, sourceRoot, targetDir });
  assertParentBoundary(session);
  if (cfg.apply && drift.unowned.length > 0) {
    throw new Error(
      `refusing to overwrite unowned scaffold path(s): ${drift.unowned.join(', ')}`,
    );
  }
  if (!cfg.apply) return finishDryRun(
    drift, removed, projectInstances, hooks, sourceRoot, targetDir,
  );
  return applyUpgrade(
    cfg, now, sourceRoot, sourcePlan, targetDir, drift, removed,
    projectInstances, hooks, expectedRoot, targetSnapshots,
  );
}

export function run(cfg, now = new Date(), hooks = {}) {
  validateHooks(hooks);
  const sourceRoot = resolve(cfg.from);
  const targetDir = resolve(cfg.target);
  assertNoSymlinkComponents(targetDir, 'target directory');
  const session = captureParentBoundary(
    targetDir, join(targetDir, '.forge-upgrade-session'), 'upgrade target session',
  );
  try { return runPinned(cfg, now, hooks, sourceRoot, targetDir, session); } finally {
    closeParentBoundary(session);
  }
}

function main(argv) {
  let cfg;
  try {
    cfg = parseArgs(argv);
  } catch (err) {
    console.error(`forge-upgrade: ${err.message}`);
    console.error(
      'usage: node harness/forge-upgrade.mjs --from <forgeos-repo> --target <project> ' +
        '[--apply] [--no-backup] [--prune]',
    );
    process.exit(2);
  }
  try {
    run(cfg);
  } catch (err) {
    console.error(`forge-upgrade: ${err.message}`);
    process.exit(1);
  }
  process.exit(0);
}

// Run only when executed directly, not on import (keeps run/classify unit-testable).
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2));
}
