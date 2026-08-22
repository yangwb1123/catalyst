// Durable ownership journal and restart recovery: dead preparing transactions
// roll back, while committed transactions only finish cleanup.
import {
  closeSync, constants, fstatSync, linkSync, lstatSync, openSync, readFileSync,
  readSync, renameSync,
} from 'node:fs';
import { randomBytes } from 'node:crypto';
import {
  basename, dirname, isAbsolute, join, relative, sep,
} from 'node:path';
import {
  anchoredPath, captureParentBoundary, cleanupUnstartedUpgradeStages, closeParentBoundary,
  recoverOwnedDirectories,
} from './upgrade-path-boundary.mjs';
import {
  assertUpgradeControls, closeUpgradeControls, finishUpgradeControls, openUpgradeControls,
  readUpgradeCleanupProof, readUpgradeControlSet, requireUpgradeMarkerIds,
  resumeUpgradeControlCleanup, upgradeMarkerId, writeUpgradeControl,
} from './transaction/upgrade-control-boundary.mjs';
import {
  findStageArtifact, randomStageArtifactPath, removeKnownFile,
} from './transaction/upgrade-stage-claim.mjs';
import { cleanupClaimedStage } from './transaction/upgrade-stage-cleanup.mjs';
import { openUpgradeRecoveryStage } from './transaction/upgrade-stage-recovery.mjs';
import {
  decodePreparedAuthority, decodeStartedAuthority,
  preparedAuthorityDocument, startedAuthorityDocument,
} from './transaction/upgrade-transaction-authority.mjs';
import { assertUpgradeJournalCapacity, decodeUpgradeStageIntents, encodeUpgradeJournal, parseUpgradeJournal } from './transaction/upgrade-stage-intent-authority.mjs';
export const UPGRADE_TRANSACTION_FILE = join('.agent', '.forge-upgrade-transaction-v1.json');
const NOFOLLOW = constants.O_NOFOLLOW;
const NONBLOCK = constants.O_NONBLOCK ?? 0;
const READ_CHUNK = 64 * 1024;
const KINDS = new Set(['added', 'backup', 'changed', 'ledger', 'project', 'removed']);
const ACTIVE_TRANSACTIONS = new Set();

function sameIdentity(left, right) { return left.dev === right.dev && left.ino === right.ino; }
export function pathError(record, detail = '') {
  return new Error(
    `refusing changed file path for ${record.kind} target ${record.rel}`
      + (detail ? `: ${detail}` : ''),
  );
}
function modeOf(stat) { return Number(stat.mode & 0o777n); }
export function requireFile(stat, identity, links, mode, record) {
  const invalid = stat.isSymbolicLink() || !stat.isFile()
    || (identity !== null && !sameIdentity(stat, identity))
    || Number(stat.nlink) !== links || (mode !== null && modeOf(stat) !== mode);
  if (invalid) throw pathError(
    record,
    `inode contract drifted (links ${Number(stat.nlink)}/${links}, mode ${
      modeOf(stat).toString(8)}/${mode === null ? '-' : mode.toString(8)})`,
  );
  return stat;
}
export function statPath(path, record, allowMissing = false) {
  try {
    return lstatSync(path, { bigint: true });
  } catch (error) {
    if (allowMissing && error?.code === 'ENOENT') return null;
    throw pathError(record, error.message);
  }
}
export function descriptorMatches(fd, expected) {
  const scratch = Buffer.allocUnsafe(Math.max(1, Math.min(READ_CHUNK, expected.length)));
  let position = 0;
  while (position < expected.length) {
    const length = Math.min(scratch.length, expected.length - position);
    const count = readSync(fd, scratch, 0, length, position);
    if (count === 0 || !expected.subarray(position, position + count)
      .equals(scratch.subarray(0, count))) return false;
    position += count;
  }
  return readSync(fd, scratch, 0, 1, position) === 0;
}

export function openSnapshot(record, keepDescriptor) {
  let fd;
  try {
    fd = openSync(record.live, constants.O_RDONLY | NOFOLLOW | NONBLOCK);
    const opened = requireFile(fstatSync(fd, { bigint: true }), null, 1, null, record);
    const lexical = requireFile(statPath(record.live, record), null, 1, null, record);
    if (!sameIdentity(opened, lexical)) throw pathError(record);
    const bytes = readFileSync(fd);
    const snapshot = {
      bytes, fd: keepDescriptor ? fd : null,
      identity: Object.freeze({ dev: opened.dev, ino: opened.ino }), mode: modeOf(opened),
    };
    if (!keepDescriptor) closeSync(fd);
    return snapshot;
  } catch (error) {
    if (fd !== undefined) {
      try { closeSync(fd); } catch {}
    }
    throw error;
  }
}

export { sameIdentity };

function safeRelative(value, label = 'transaction path') {
  if (typeof value !== 'string' || value.length === 0 || isAbsolute(value)
      || value.includes('\\') || value.split('/').some((part) => !part || part === '.' || part === '..')) {
    throw new Error(`unsafe ${label}: ${String(value)}`);
  }
  if (relative('.', value).split(sep).join('/') !== value) {
    throw new Error(`noncanonical ${label}: ${value}`);
  }
  return value;
}

function stageName(owner, index) {
  return `.forge-upgrade-txn-${owner.slice(0, 12)}-${String(index).padStart(4, '0')}`;
}

function processStart(pid) {
  try {
    const text = readFileSync(`/proc/${pid}/stat`, 'utf8');
    const fields = text.slice(text.lastIndexOf(')') + 2).trim().split(/\s+/);
    return fields[19] ?? null;
  } catch {
    return null;
  }
}

function ownerIsAlive(owner, transactionId) {
  if (owner.pid === process.pid) return ACTIVE_TRANSACTIONS.has(transactionId);
  try { process.kill(owner.pid, 0); } catch (error) {
    if (error?.code === 'ESRCH') return false;
    return true;
  }
  const current = processStart(owner.pid);
  return owner.process_start === null || current === null || current === owner.process_start;
}

function entry(kind, rel, owner, index, sourceRel = null) {
  safeRelative(rel);
  if (!KINDS.has(kind)) throw new Error(`invalid transaction kind: ${kind}`);
  return {
    kind, rel, source_rel: sourceRel, stage_name: stageName(owner, index),
  };
}

function plannedEntries(spec, owner) {
  const entries = [];
  const append = (kind, rel, sourceRel = null) => {
    entries.push(entry(kind, rel, owner, entries.length, sourceRel));
  };
  for (const rel of spec.added) append('added', rel);
  for (const rel of spec.changed) append('changed', rel);
  for (const rel of spec.removed) append('removed', rel);
  for (const rel of spec.projectInstances) append('project', rel);
  for (const rel of spec.backups ?? []) {
    append('backup', join(spec.backupRoot, rel).split(sep).join('/'), rel);
  }
  append('ledger', relative(spec.targetDir, spec.statePath).split(sep).join('/'));
  const keys = entries.map((item) => item.rel);
  if (new Set(keys).size !== keys.length) throw new Error('duplicate upgrade transaction entry');
  return entries;
}

function directoryProjection(entries) {
  const directories = new Set();
  for (const item of entries) {
    let parent = dirname(item.rel).split(sep).join('/');
    while (parent !== '.') {
      directories.add(parent);
      parent = dirname(parent).split(sep).join('/');
    }
  }
  return [...directories].sort();
}

function encoded(document) {
  return Buffer.from(`${JSON.stringify(document, null, 2)}\n`);
}

function optionalStat(path) {
  try {
    const stat = lstatSync(path, { bigint: true });
    if (stat.isSymbolicLink() || !stat.isFile()) throw new Error(`unsafe recovery file: ${path}`);
    return stat;
  } catch (error) {
    if (error?.code === 'ENOENT') return null;
    throw error;
  }
}

function exactKeys(value, expected) {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    && JSON.stringify(Object.keys(value).sort()) === JSON.stringify(expected);
}

function decimal(value, positive = false) {
  return typeof value === 'string'
    && (positive ? /^[1-9]\d*$/.test(value) : /^(0|[1-9]\d*)$/.test(value));
}

function validateDocument(document, targetIdentity) {
  const keys = Object.keys(document ?? {}).sort();
  const expected = [
    'api_version', 'directories', 'entries', 'owner', 'phase', 'target', 'transaction_id',
  ];
  if (JSON.stringify(keys) !== JSON.stringify(expected)
      || document.api_version !== 'forgeos.scaffold-upgrade-transaction/v1'
      || !/^[a-f0-9]{32}$/.test(document.transaction_id)
      || document.phase !== 'preparing'
      || !exactKeys(document.owner, ['pid', 'process_start'])
      || !Number.isSafeInteger(document.owner?.pid) || document.owner.pid <= 0
      || (document.owner.process_start !== null
        && !decimal(document.owner.process_start, true))
      || !exactKeys(document.target, ['dev', 'ino'])
      || !decimal(document.target.dev) || !decimal(document.target.ino, true)) {
    throw new Error('malformed scaffold upgrade transaction journal');
  }
  if (document.target.dev !== String(targetIdentity.dev)
      || document.target.ino !== String(targetIdentity.ino)) {
    throw new Error('scaffold upgrade transaction target identity changed');
  }
  if (!Array.isArray(document.entries) || !Array.isArray(document.directories)) {
    throw new Error('malformed scaffold upgrade transaction projection');
  }
  document.entries.forEach((item, index) => validateEntry(item, document.transaction_id, index));
  for (const value of document.directories) safeRelative(value, 'transaction directory');
  const entryKeys = document.entries.map((item) => item.rel);
  if (new Set(entryKeys).size !== entryKeys.length
      || JSON.stringify(document.directories) !== JSON.stringify(directoryProjection(document.entries))) {
    throw new Error('malformed scaffold upgrade transaction projection');
  }
  return document;
}

function validateEntry(item, owner, index) {
  const keys = Object.keys(item ?? {}).sort();
  if (JSON.stringify(keys) !== JSON.stringify(['kind', 'rel', 'source_rel', 'stage_name'])
      || !KINDS.has(item.kind) || item.stage_name !== stageName(owner, index)) {
    throw new Error('malformed scaffold upgrade transaction entry');
  }
  safeRelative(item.rel);
  if (item.source_rel !== null) safeRelative(item.source_rel, 'backup source path');
  if ((item.kind === 'backup') !== (item.source_rel !== null)) {
    throw new Error('malformed scaffold upgrade backup entry');
  }
}

function restoreCaptured(captured, live, next, rel, directoryFd) {
  const stat = optionalStat(captured);
  if (stat === null) return null;
  if (next !== null && sameIdentity(stat, next)) {
    removeKnownFile(captured, next, `rollback successor ${rel}`, false, directoryFd);
    return null;
  }
  try {
    linkSync(captured, live);
    removeKnownFile(captured, stat, `rollback replacement ${rel}`, false, directoryFd);
  } catch (error) {
    throw new Error(`preserved concurrent replacement for ${rel} at ${captured}: ${error.message}`);
  }
  return stat;
}

function requireRecoverablePriorLive(stage, live, prior, next, entryValue) {
  const path = findStageArtifact(
    stage.path, 'prior-live', `prior-live capture for ${entryValue.rel}`,
  );
  if (path === null) return;
  const stat = optionalStat(path);
  if (stat === null || (prior !== null && sameIdentity(stat, prior))) return;
  if (next !== null && sameIdentity(stat, next)) {
    removeKnownFile(path, next, `prior-live successor ${entryValue.rel}`, false, stage.fd);
    return;
  }
  if (optionalStat(live) === null) {
    restoreCaptured(path, live, next, entryValue.rel, stage.fd);
  }
  throw new Error(`preserved concurrent replacement for ${entryValue.rel} at ${path}`);
}

function rollbackLive(live, stage, entryValue) {
  const nextPath = join(stage.path, 'next');
  const priorPath = join(stage.path, 'prior');
  let captured = findStageArtifact(
    stage.path, 'rollback-live', `rollback capture for ${entryValue.rel}`,
  );
  const next = optionalStat(nextPath);
  const prior = optionalStat(priorPath);
  requireRecoverablePriorLive(stage, live, prior, next, entryValue);
  if (captured === null) {
    captured = randomStageArtifactPath(stage.path, 'rollback-live');
    try { renameSync(live, captured); } catch (error) {
      if (error?.code !== 'ENOENT') throw error;
    }
  } else if (optionalStat(live) !== null) {
    throw new Error(`ambiguous interrupted rollback for ${entryValue.rel}`);
  }
  restoreCaptured(captured, live, next, entryValue.rel, stage.fd);
  if (prior !== null && optionalStat(live) === null) linkSync(priorPath, live);
  const current = optionalStat(live);
  if (prior !== null && (current === null || !sameIdentity(current, prior))) {
    throw new Error(`preserved prior ${entryValue.rel} at ${priorPath}`);
  }
}

function recoverEntry(
  targetDir, entryValue, authority, committed, expectedRoot, hooks,
) {
  const displayed = join(targetDir, entryValue.rel);
  let boundary;
  try {
    boundary = captureParentBoundary(targetDir, displayed,
      `transaction recovery ${entryValue.rel}`, { expectedRoot });
  } catch (error) {
    if (error?.code === 'ENOENT') return;
    throw error;
  }
  let stage;
  try {
    stage = openUpgradeRecoveryStage(boundary, entryValue, authority);
    if (stage === null) return;
    if (!committed) rollbackLive(anchoredPath(boundary, basename(displayed)), stage, entryValue);
    cleanupClaimedStage(stage, hooks);
    stage = null;
  } finally {
    if (stage?.fd !== null && stage?.fd !== undefined) closeSync(stage.fd);
    closeParentBoundary(boundary);
  }
}
function parseRecoveryJournal(record, targetIdentity) {
  const parsed = parseUpgradeJournal(record.bytes);
  const document = validateDocument(parsed.document, targetIdentity);
  return {
    document, intents: decodeUpgradeStageIntents(parsed.intentDocuments, document),
  };
}
function recoverFinished(control, state) {
  const id = upgradeMarkerId(state.finished, 'finish marker');
  if (state.journal !== null) {
    const { document } = parseRecoveryJournal(state.journal, control.targetIdentity);
    if (document.transaction_id !== id) throw new Error('scaffold upgrade finish marker mismatch');
  }
  requireUpgradeMarkerIds(state, id);
  finishUpgradeControls(control, id);
  return { phase: 'finishing', recovered: true, transactionId: id };
}
function cleanupUnstarted(control, state, document, intents, targetDir, hooks) {
  const authorities = state.prepared === null ? null : decodePreparedAuthority(
    state.prepared, state.journal, document,
  );
  cleanupUnstartedUpgradeStages(
    targetDir, document, authorities, intents, hooks, control.targetIdentity,
  );
  finishUpgradeControls(control, document.transaction_id);
  return { phase: 'unstarted', recovered: true, transactionId: document.transaction_id };
}
function recoverStarted(control, state, document, targetDir, hooks) {
  if (state.prepared === null) {
    throw new Error('missing scaffold upgrade prepared authority');
  }
  decodeStartedAuthority(
    state.started, state.journal, state.prepared, document.transaction_id,
  );
  const authorities = decodePreparedAuthority(
    state.prepared, state.journal, document,
  );
  const committed = state.committed !== null;
  for (const item of [...document.entries].reverse()) {
    recoverEntry(
      targetDir, item, authorities.get(item.stage_name), committed,
      control.targetIdentity, hooks,
    );
  }
  recoverOwnedDirectories(
    targetDir, document.directories, document.transaction_id, !committed, hooks,
    control.targetIdentity,
  );
  finishUpgradeControls(control, document.transaction_id);
  return {
    phase: committed ? 'committed' : 'preparing', recovered: true,
    transactionId: document.transaction_id,
  };
}
export function recoverInterruptedUpgrade(targetDir, hooks = {}, expectedRoot = null) {
  const control = openUpgradeControls(targetDir, UPGRADE_TRANSACTION_FILE);
  try {
    if (expectedRoot !== null && !sameIdentity(control.targetIdentity, expectedRoot))
      throw new Error('scaffold upgrade target changed before recovery');
    const state = readUpgradeControlSet(control);
    if (state.journal === null) {
      if (state.started !== null || state.prepared !== null
          || state.committed !== null || state.finished !== null) {
        throw new Error('orphaned scaffold upgrade transaction marker');
      }
      closeUpgradeControls(control);
      return null;
    }
    const { document, intents } = parseRecoveryJournal(
      state.journal, control.targetIdentity,
    );
    requireUpgradeMarkerIds(state, document.transaction_id);
    if (ownerIsAlive(document.owner, document.transaction_id))
      throw new Error('another scaffold upgrade transaction is active');
    const cleanup = readUpgradeCleanupProof(control, {
      controls: control.identities,
      journalIdentity: state.journal.identity,
      transactionId: document.transaction_id,
    });
    if (cleanup !== null) {
      if (cleanup.transactionId !== document.transaction_id) {
        throw new Error('scaffold upgrade cleanup transaction mismatch');
      }
      resumeUpgradeControlCleanup(control, cleanup, hooks);
      return {
        phase: 'finishing', recovered: true, transactionId: cleanup.transactionId,
      };
    }
    if (state.finished !== null) return recoverFinished(control, state);
    if (state.started === null) {
      if (state.committed !== null) {
        throw new Error('orphaned scaffold upgrade commit marker');
      }
      return cleanupUnstarted(control, state, document, intents, targetDir, hooks);
    }
    return recoverStarted(control, state, document, targetDir, hooks);
  } finally {
    closeUpgradeControls(control);
  }
}
function cancelTransactionBegin(control, transactionId, cause) {
  try { finishUpgradeControls(control, transactionId, {}, true); } catch (error) {
    throw new Error(`${cause.message}; transaction control cleanup failed: ${error.message}`);
  }
  throw cause;
}
export function beginUpgradeTransaction(spec, hooks = {}) {
  const id = randomBytes(16).toString('hex');
  const entries = plannedEntries(spec, id);
  const control = openUpgradeControls(spec.targetDir, UPGRADE_TRANSACTION_FILE);
  if (spec.expectedRoot !== undefined && !sameIdentity(control.targetIdentity, spec.expectedRoot)) {
    closeUpgradeControls(control);
    throw new Error('scaffold upgrade target changed after classification');
  }
  const document = {
    api_version: 'forgeos.scaffold-upgrade-transaction/v1',
    directories: directoryProjection(entries),
    entries,
    owner: { pid: process.pid, process_start: processStart(process.pid) },
    phase: 'preparing',
    target: { dev: String(control.targetIdentity.dev), ino: String(control.targetIdentity.ino) },
    transaction_id: id,
  };
  try { assertUpgradeJournalCapacity(document); } catch (error) { closeUpgradeControls(control); throw error; }
  ACTIVE_TRANSACTIONS.add(id);
  try {
    writeUpgradeControl(control, 'journal', encodeUpgradeJournal(document), false, { hooks });
    hooks.afterTransactionJournalWrite?.({ path: join(spec.targetDir, UPGRADE_TRANSACTION_FILE) });
    assertUpgradeControls(control);
  } catch (error) { ACTIVE_TRANSACTIONS.delete(id); cancelTransactionBegin(control, id, error); }
  return {
    control, document,
    entries: new Map(entries.map((item) => [`${item.kind}\0${item.rel}`, item])),
    hooks, id, stageIntents: new Map(), deactivate() { ACTIVE_TRANSACTIONS.delete(id); },
  };
}
export function startUpgradeTransaction(transaction, claims) {
  if (transaction.stageIntents.size !== transaction.document.entries.length) {
    throw new Error('incomplete scaffold upgrade stage intent authority');
  }
  const prepared = preparedAuthorityDocument(transaction, claims);
  const preparedIdentity = writeUpgradeControl(
    transaction.control, 'prepared', encoded(prepared), false,
    { hooks: transaction.hooks },
  );
  const started = startedAuthorityDocument(transaction, preparedIdentity);
  writeUpgradeControl(
    transaction.control, 'started', encoded(started), false,
    { hooks: transaction.hooks },
  );
  transaction.hooks.afterTransactionStart?.();
}

export function transactionEntry(transaction, kind, rel) {
  const value = transaction.entries.get(`${kind}\0${rel}`);
  if (value === undefined) throw new Error(`missing transaction plan for ${kind} ${rel}`);
  return value;
}
export function markUpgradeTransactionCommitted(transaction) {
  writeUpgradeControl(
    transaction.control, 'committed', Buffer.from(`${transaction.id}\n`), false,
    {
      hooks: transaction.hooks,
      onDurable() { transaction.committed = true; },
    },
  );
}

export function finishUpgradeTransaction(transaction) {
  try { finishUpgradeControls(transaction.control, transaction.id, transaction.hooks); }
  finally { transaction.deactivate(); }
}
