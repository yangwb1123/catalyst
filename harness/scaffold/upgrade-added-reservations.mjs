// Descriptor-pinned transaction with retained inode proofs for every target.
import { fstatSync, linkSync, renameSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';
import {
  anchoredPath, assertParentBoundary, captureParentBoundary,
  cleanupOwnedBoundaryDirectories, closeParentBoundary,
} from './upgrade-path-boundary.mjs';
import {
  confirmLedgerReservation, prepareLedgerReservation, publishLedgerReservation,
  releaseLedgerReservation, rollbackLedgerReservation,
} from './upgrade-ledger-reservation.mjs';
import {
  beginUpgradeTransaction, descriptorMatches, finishUpgradeTransaction,
  markUpgradeTransactionCommitted, openSnapshot, pathError, requireFile,
  sameIdentity, startUpgradeTransaction, statPath, transactionEntry,
} from './upgrade-transaction-journal.mjs';
import { syncUpgradePublication } from './transaction/upgrade-durability.mjs';
import { cleanupUpgradeRecord } from './transaction/upgrade-record-cleanup.mjs';
import { requireClassifiedSnapshot } from './transaction/upgrade-target-snapshot.mjs';
import { createTransactionClaim, randomStageArtifactPath, removeKnownFile,
  writePriorClaim } from './transaction/upgrade-stage-claim.mjs';
import { appendUpgradeStageIntent } from './transaction/upgrade-stage-intent-authority.mjs';
function sourceSnapshot(sourceSnapshots, rel) {
  const snapshot = sourceSnapshots.get(rel);
  if (snapshot === undefined) throw new Error(`missing frozen source snapshot: ${rel}`);
  return snapshot;
}
function boundaryFor(transaction, destination, rel) {
  const parent = dirname(destination);
  let boundary = transaction.boundaries.get(parent);
  if (boundary !== undefined) return boundary;
  boundary = captureParentBoundary(
    transaction.targetDir, destination, `upgrade target ${rel}`,
    { create: true, expectedRoot: transaction.journal.control.targetIdentity,
      owner: transaction.journal.id },
  );
  transaction.boundaries.set(parent, boundary);
  return boundary;
}
function stagedClaim(record, bytes, mode, transaction) {
  const claim = createTransactionClaim(
    record.boundary, record.plan.stage_name, bytes, mode,
    `${record.kind} target ${record.rel} stage`, transaction.hooks,
    (intent) => appendUpgradeStageIntent(transaction.journal, intent),
  );
  return { claim, stage: claim.stage };
}
function expectAbsent(record) {
  const stat = statPath(record.live, record, true);
  if (stat !== null) {
    if (record.kind === 'added') {
      throw new Error(`refusing to overwrite unowned scaffold path(s): ${record.rel}`);
    }
    throw pathError(record, 'expected an absent destination');
  }
}
function attachPrior(record, hooks) {
  writePriorClaim(record.claim, record.prior, hooks);
  record.priorSentinel = join(record.claim.directory, 'prior');
  linkSync(record.live, record.priorSentinel);
  requireFile(statPath(record.priorSentinel, record), record.prior.identity,
    2, record.prior.mode, record);
  requireFile(fstatSync(record.prior.fd, { bigint: true }), record.prior.identity,
    2, record.prior.mode, record);
}
function prepareRecord(transaction, kind, rel) {
  const displayed = join(transaction.targetDir, rel);
  const boundary = boundaryFor(transaction, displayed, rel);
  const record = {
    boundary, claim: null, displayed, kind, live: anchoredPath(boundary, basename(displayed)),
    mode: 0o600, prior: null, priorDetached: false, priorQuarantine: null,
    priorSentinel: null, proof: null, published: false, rel, skipped: false,
    plan: transactionEntry(transaction.journal, kind, rel),
    stage: null,
  };
  transaction.records.push(record);
  const source = kind === 'removed'
    ? { bytes: Buffer.alloc(0), mode: 0o600 }
    : sourceSnapshot(transaction.sourceSnapshots, rel);
  record.expected = source.bytes;
  if (kind === 'changed' || kind === 'removed') {
    record.prior = openSnapshot(record, true);
    requireClassifiedSnapshot(transaction.targetSnapshots, record, record.prior);
  }
  record.mode = kind === 'changed' ? record.prior.mode : source.mode;
  ({ claim: record.claim, stage: record.stage } = stagedClaim(
    record, source.bytes, record.mode, transaction,
  ));
  if (kind === 'changed' || kind === 'removed') attachPrior(record, transaction.hooks);
  else expectAbsent(record);
}
function prepareBackupRecord(transaction, sourceRecord) {
  const plan = [...transaction.journal.entries.values()].find(
    (item) => item.kind === 'backup' && item.source_rel === sourceRecord.rel,
  );
  if (plan === undefined) throw new Error(`missing backup transaction plan for ${sourceRecord.rel}`);
  const displayed = join(transaction.targetDir, plan.rel);
  const boundary = boundaryFor(transaction, displayed, plan.rel);
  const record = {
    boundary, claim: null, displayed, expected: sourceRecord.prior.bytes,
    kind: 'backup', live: anchoredPath(boundary, basename(displayed)),
    mode: sourceRecord.prior.mode, plan, prior: null, priorDetached: false,
    priorQuarantine: null, priorSentinel: null, proof: null, published: false,
    rel: plan.rel, skipped: false, sourceRel: sourceRecord.rel, stage: null,
  };
  transaction.backups.push(record);
  ({ claim: record.claim, stage: record.stage } = stagedClaim(
    record, record.expected, record.mode, transaction,
  ));
  expectAbsent(record);
}
function prepareObservation(transaction, rel) {
  const displayed = join(transaction.targetDir, rel);
  const boundary = boundaryFor(transaction, displayed, rel);
  const record = {
    boundary, displayed, kind: 'observed',
    live: anchoredPath(boundary, basename(displayed)), rel,
  };
  record.snapshot = requireClassifiedSnapshot(
    transaction.targetSnapshots, record, openSnapshot(record, false),
  );
  transaction.observed.push(record);
}
function remember(errors, error) { errors.push(error instanceof Error ? error : new Error(String(error))); }
function closeBoundaries(transaction, errors) {
  for (const boundary of [...transaction.boundaries.values()].reverse()) {
    try { closeParentBoundary(boundary); } catch (error) { remember(errors, error); }
  }
}
function assertStage(record, links, paths = []) {
  const identity = record.claim.identity;
  const opened = requireFile(fstatSync(record.claim.fd, { bigint: true }),
    identity, links, record.mode, record);
  for (const path of [record.stage, record.claim.sentinel, ...paths]) {
    requireFile(statPath(path, record), identity, links, record.mode, record);
  }
  if (Number(opened.size) !== record.expected.length
      || !descriptorMatches(record.claim.fd, record.expected)) throw pathError(record);
}

function assertPrior(record, paths) {
  const prior = record.prior;
  const opened = requireFile(fstatSync(prior.fd, { bigint: true }),
    prior.identity, 2, prior.mode, record);
  for (const path of paths) {
    requireFile(statPath(path, record), prior.identity, 2, prior.mode, record);
  }
  if (Number(opened.size) !== prior.bytes.length
      || !descriptorMatches(prior.fd, prior.bytes)) throw pathError(record);
}

function assertObservation(record) {
  const current = openSnapshot(record, false);
  if (!sameIdentity(current.identity, record.snapshot.identity)
      || current.mode !== record.snapshot.mode
      || !current.bytes.equals(record.snapshot.bytes)) throw pathError(record);
}

function assertPreparedRecord(record) {
  assertParentBoundary(record.boundary);
  assertStage(record, 1);
  if (record.prior !== null) {
    assertPrior(record, [record.live, record.priorSentinel]);
  } else if (record.kind === 'project') {
    const current = statPath(record.live, record, true);
    if (current !== null) openSnapshot(record, false);
  } else {
    expectAbsent(record);
  }
}

function allRecords(transaction) {
  return [...transaction.backups, ...transaction.records];
}
function assertPrepared(transaction) {
  for (const boundary of transaction.boundaries.values()) assertParentBoundary(boundary);
  for (const record of transaction.observed) assertObservation(record);
  for (const record of allRecords(transaction)) assertPreparedRecord(record);
}

function restoreReplacement(quarantine, live, record, errors) {
  try {
    const captured = statPath(quarantine, record);
    linkSync(quarantine, live);
    removeKnownFile(
      quarantine, captured, `concurrent replacement ${record.rel}`, false,
      record.claim.directoryFd,
    );
    return true;
  } catch (error) {
    remember(errors, new Error(
      `preserved concurrent replacement for ${record.rel} at ${quarantine}: ${error.message}`,
    ));
    return false;
  }
}

function detachPrior(record, transaction) {
  assertParentBoundary(record.boundary);
  transaction.hooks.beforePriorDetach?.({ destination: record.displayed, rel: record.rel });
  record.priorQuarantine = randomStageArtifactPath(
    record.claim.directory, 'prior-live',
  );
  renameSync(record.live, record.priorQuarantine);
  record.priorDetached = true;
  transaction.hooks.afterPriorDetach?.({ destination: record.displayed, rel: record.rel });
  const captured = statPath(record.priorQuarantine, record);
  if (!sameIdentity(captured, record.prior.identity)) {
    const errors = [];
    if (restoreReplacement(record.priorQuarantine, record.live, record, errors)) {
      record.priorQuarantine = null;
    }
    record.preservePrior = true;
    throw pathError(record, `pathname changed before publication${
      errors.length ? `; ${errors[0].message}` : ''}`);
  }
  assertPrior(record, [record.priorSentinel, record.priorQuarantine]);
}

function publishNew(record, allowExisting) {
  try {
    linkSync(record.claim.sentinel, record.live);
  } catch (error) {
    if (error?.code !== 'EEXIST' || !allowExisting) {
      if (record.kind === 'added' && error?.code === 'EEXIST') {
        throw new Error(`refusing to overwrite unowned scaffold path(s): ${record.rel}`);
      }
      throw error;
    }
    openSnapshot(record, false);
    record.skipped = true;
    return;
  }
  record.published = true;
  assertStage(record, 2, [record.live]);
}

function publishRecords(transaction) {
  for (const record of allRecords(transaction)) {
    assertParentBoundary(record.boundary);
    if (record.kind === 'added' || record.kind === 'backup') {
      publishNew(record, false);
      if (record.kind === 'added') {
        transaction.hooks.afterAddedReservation?.({
          destination: record.displayed, identity: record.claim.identity, rel: record.rel,
        });
      } else {
        transaction.hooks.afterBackupReservation?.({
          destination: record.displayed, rel: record.rel, sourceRel: record.sourceRel,
        });
      }
      assertStage(record, 2, [record.live]);
    } else if (record.kind === 'changed') {
      detachPrior(record, transaction);
      publishNew(record, false);
    } else if (record.kind === 'removed') {
      detachPrior(record, transaction);
    } else {
      publishNew(record, true);
    }
    assertParentBoundary(record.boundary);
  }
}

function assertPublishedRecord(record, confirmed) {
  const newPaths = [record.live];
  if (confirmed) newPaths.push(record.proof);
  if (record.published) assertStage(record, confirmed ? 3 : 2, newPaths);
  else assertStage(record, 1);
  if (record.priorDetached) {
    assertPrior(record, [record.priorSentinel, record.priorQuarantine]);
    if (record.kind === 'removed' && statPath(record.live, record, true) !== null) {
      throw pathError(record, 'retired destination reappeared');
    }
  }
  if (record.skipped) openSnapshot(record, false);
  assertParentBoundary(record.boundary);
}

function assertPublished(transaction, confirmed = false) {
  for (const record of transaction.observed) assertObservation(record);
  for (const record of allRecords(transaction)) assertPublishedRecord(record, confirmed);
  for (const boundary of transaction.boundaries.values()) assertParentBoundary(boundary);
}

function confirmRecords(transaction) {
  for (const record of allRecords(transaction)) {
    if (!record.published) continue;
    record.proof = join(record.claim.directory, 'commit-proof');
    linkSync(record.live, record.proof);
  }
  assertPublished(transaction, true);
  transaction.confirmed = true;
}

function quarantineLive(record, transaction, errors) {
  const quarantine = randomStageArtifactPath(
    record.claim.directory, 'rollback-live',
  );
  if (record.kind === 'added') {
    try {
      transaction.hooks.beforeAddedRollbackRename?.({
        destination: record.displayed, identity: record.claim.identity, rel: record.rel,
      });
    } catch (error) { remember(errors, error); }
  }
  try { renameSync(record.live, quarantine); } catch (error) {
    if (error?.code !== 'ENOENT') remember(errors, error);
    return true;
  }
  const captured = statPath(quarantine, record);
  if (sameIdentity(captured, record.claim.identity)) {
    try {
      removeKnownFile(
        quarantine, record.claim.identity, `rollback ${record.rel}`, false,
        record.claim.directoryFd,
      );
    } catch (error) { remember(errors, error); }
    return true;
  }
  restoreReplacement(quarantine, record.live, record, errors);
  return false;
}

function priorStillRecoverable(record, errors) {
  const live = statPath(record.live, record, true);
  if (live !== null && sameIdentity(live, record.prior.identity)) return false;
  if (live === null) {
    try {
      linkSync(record.priorSentinel, record.live);
      return false;
    } catch (error) { remember(errors, error); }
  }
  remember(errors, new Error(`preserved prior ${record.rel} at ${record.priorSentinel}`));
  return true;
}

function rollbackRecords(transaction) {
  const errors = [];
  for (const record of allRecords(transaction).reverse()) {
    let liveVacant = true;
    if (record.published || record.priorDetached) {
      liveVacant = quarantineLive(record, transaction, errors);
    }
    let preservePrior = Boolean(record.preservePrior);
    if (record.prior !== null && record.claim !== null) {
      if (record.priorDetached && liveVacant) {
        try { linkSync(record.priorSentinel, record.live); } catch (error) {
          preservePrior = true;
          remember(errors, error);
        }
      } else if (record.priorDetached || priorStillRecoverable(record, errors)) {
        preservePrior = true;
      }
    }
    if (record.preservePrior) remember(
      errors, new Error(`preserved unresolved transaction evidence for ${record.rel}`),
    );
    cleanupUpgradeRecord(record, preservePrior, errors, transaction.hooks);
  }
  try {
    cleanupOwnedBoundaryDirectories(
      transaction.boundaries.values(), transaction.journal.id, true, transaction.hooks,
    );
  } catch (error) { remember(errors, error); }
  closeBoundaries(transaction, errors);
  return errors;
}

function releaseRecords(transaction) {
  const errors = [];
  for (const record of allRecords(transaction)) {
    cleanupUpgradeRecord(record, false, errors, transaction.hooks);
  }
  try {
    cleanupOwnedBoundaryDirectories(
      transaction.boundaries.values(), transaction.journal.id, false, transaction.hooks,
    );
  } catch (error) { remember(errors, error); }
  closeBoundaries(transaction, errors);
  return errors;
}

function prepareTransaction(spec, hooks) {
  const journal = beginUpgradeTransaction(spec, hooks);
  const transaction = {
    backups: [], boundaries: new Map(), confirmed: false, hooks: hooks ?? {},
    journal, observed: [], records: [], sourceSnapshots: spec.sourceSnapshots,
    targetDir: spec.targetDir, targetSnapshots: spec.targetSnapshots,
  };
  try {
    for (const rel of spec.added) prepareRecord(transaction, 'added', rel);
    for (const rel of spec.changed) prepareRecord(transaction, 'changed', rel);
    for (const rel of spec.removed) prepareRecord(transaction, 'removed', rel);
    for (const rel of spec.projectInstances) prepareRecord(transaction, 'project', rel);
    for (const rel of spec.backups ?? []) {
      const source = transaction.records.find((record) => record.rel === rel);
      if (source?.prior === null || source === undefined) {
        throw new Error(`backup source was not prepared: ${rel}`);
      }
      prepareBackupRecord(transaction, source);
    }
    for (const rel of spec.observed) prepareObservation(transaction, rel);
    assertPrepared(transaction);
    return transaction;
  } catch (error) {
    const failures = rollbackRecords(transaction);
    if (failures.length === 0) {
      try { finishUpgradeTransaction(journal); } catch (cleanup) { failures.push(cleanup); }
    }
    if (failures.length) throw new Error(
      `${error.message}; reservation cleanup failed: ${failures.map((item) => item.message).join('; ')}`,
    );
    throw error;
  }
}

function rollbackTransaction(transaction, ledger, cause) {
  const errors = [];
  if (ledger !== null) {
    try { rollbackLedgerReservation(ledger); } catch (error) { remember(errors, error); }
  }
  errors.push(...rollbackRecords(transaction));
  if (errors.length === 0) {
    try { finishUpgradeTransaction(transaction.journal); } catch (error) { remember(errors, error); }
  }
  if (errors.length) throw new Error(
    `${cause.message}; upgrade rollback failed: ${errors.map((item) => item.message).join('; ')}`,
  );
  throw cause;
}

function releaseTransaction(transaction, ledger) {
  const errors = [];
  try { releaseLedgerReservation(ledger); } catch (error) { remember(errors, error); }
  errors.push(...releaseRecords(transaction));
  if (errors.length === 0) {
    try { finishUpgradeTransaction(transaction.journal); } catch (error) { remember(errors, error); }
  }
  if (errors.length) throw new Error(
    `upgrade committed but cleanup failed: ${errors.map((item) => item.message).join('; ')}`,
  );
}

function preparedClaims(transaction, ledger) {
  return new Map([
    ...allRecords(transaction).map(
      (record) => [record.plan.stage_name, record.claim],
    ),
    [ledger.claim.stageName, ledger.claim],
  ]);
}
export function withAddedReservations(spec, hooks) {
  const transaction = prepareTransaction(spec, hooks);
  try {
    let ledger = null;
    try {
      const ledgerPlan = [...transaction.journal.entries.values()].find(
        (item) => item.kind === 'ledger',
      );
      if (ledgerPlan === undefined) throw new Error('missing scaffold-state transaction plan');
      ledger = prepareLedgerReservation(
        spec.targetDir, spec.statePath, spec.stateContent,
        ledgerPlan.stage_name, transaction.journal.control.targetIdentity,
        transaction.hooks,
        (intent) => appendUpgradeStageIntent(transaction.journal, intent),
      );
      startUpgradeTransaction(
        transaction.journal, preparedClaims(transaction, ledger),
      );
      transaction.hooks.afterClassification?.(spec.hookPayload);
      assertPrepared(transaction);
      publishRecords(transaction);
      assertPublished(transaction);
      publishLedgerReservation(ledger);
      transaction.hooks.afterScaffoldStateWrite?.({ path: spec.statePath });
      confirmRecords(transaction);
      confirmLedgerReservation(ledger);
      syncUpgradePublication(
        [...transaction.boundaries.values(), ledger.boundary],
        [...allRecords(transaction).map((record) => record.claim), ledger.claim],
      );
      transaction.hooks.afterTransactionDurability?.();
      markUpgradeTransactionCommitted(transaction.journal);
      transaction.hooks.afterTransactionCommit?.({ path: spec.statePath });
    } catch (error) {
      if (transaction.journal.committed) {
        releaseTransaction(transaction, ledger);
        throw new Error(`upgrade committed but durability confirmation failed: ${error.message}`);
      }
      rollbackTransaction(transaction, ledger, error);
    }
    const projectInitialized = transaction.records.filter(
      (record) => record.kind === 'project' && record.published).length;
    const stateUpdated = !ledger.unchanged;
    releaseTransaction(transaction, ledger);
    return { backedUp: transaction.backups.length, projectInitialized, stateUpdated };
  } finally { transaction.journal.deactivate(); }
}
