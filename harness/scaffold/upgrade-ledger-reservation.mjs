// Descriptor-backed scaffold-state commit/rollback for forge-upgrade.
// The prior ledger and staged successor retain private hardlinks until both the
// ledger pathname and every newly added destination have post-publication proof.
import {
  closeSync,
  constants,
  fstatSync,
  linkSync,
  lstatSync,
  openSync,
  readFileSync,
  readSync,
  renameSync,
} from 'node:fs';
import { basename, join } from 'node:path';

import {
  anchoredPath,
  assertParentBoundary,
  captureParentBoundary,
  closeParentBoundary,
} from './upgrade-path-boundary.mjs';
import {
  createTransactionClaim, randomStageArtifactPath, removeKnownFile,
  writePriorClaim,
} from './transaction/upgrade-stage-claim.mjs';
import {
  cleanupClaimArtifacts, removeClaimDirectory,
} from './transaction/upgrade-stage-cleanup.mjs';

const READ_CHUNK = 64 * 1024;
const NOFOLLOW = constants.O_NOFOLLOW;
const NONBLOCK = constants.O_NONBLOCK ?? 0;

function sameIdentity(left, right) {
  return left.dev === right.dev && left.ino === right.ino;
}

function changedLedger(detail = '') {
  return new Error(
    `refusing changed scaffold-state ledger${detail ? `: ${detail}` : ''}`,
  );
}

function ledgerStat(path) {
  try {
    return lstatSync(path, { bigint: true });
  } catch (error) {
    throw changedLedger(error.message);
  }
}

function requireLedgerStat(stat, identity, links, mode = null) {
  const permissions = Number(stat.mode & 0o777n);
  if (stat.isSymbolicLink() || !stat.isFile()
      || !sameIdentity(stat, identity) || Number(stat.nlink) !== links
      || (mode !== null && permissions !== mode)) {
    throw changedLedger();
  }
  return stat;
}

function descriptorMatches(fd, expected) {
  const scratch = Buffer.allocUnsafe(Math.max(1, Math.min(READ_CHUNK, expected.length)));
  let position = 0;
  while (position < expected.length) {
    const length = Math.min(scratch.length, expected.length - position);
    const count = readSync(fd, scratch, 0, length, position);
    if (count === 0) return false;
    if (!expected.subarray(position, position + count).equals(scratch.subarray(0, count))) {
      return false;
    }
    position += count;
  }
  return readSync(fd, scratch, 0, 1, position) === 0;
}

function openPriorLedger(path) {
  let fd;
  try {
    fd = openSync(path, constants.O_RDONLY | NOFOLLOW | NONBLOCK);
    const opened = fstatSync(fd, { bigint: true });
    const lexical = ledgerStat(path);
    requireLedgerStat(opened, lexical, 1);
    const bytes = readFileSync(fd);
    return {
      bytes,
      fd,
      identity: Object.freeze({ dev: opened.dev, ino: opened.ino }),
      mode: Number(opened.mode & 0o777n),
    };
  } catch (error) {
    if (fd !== undefined) closeSync(fd);
    throw error;
  }
}

function stagedLedger(boundary, content, mode, stageName, hooks, authorize) {
  return createTransactionClaim(
    boundary, stageName, content, mode, 'scaffold-state staged successor', hooks,
    authorize,
  );
}

function remember(errors, error) {
  errors.push(error instanceof Error ? error : new Error(String(error)));
}

function close(fd, errors) {
  if (fd === null) return;
  try { closeSync(fd); } catch (error) { remember(errors, error); }
}

function restoreReplacement(record, quarantine, errors) {
  try {
    const captured = ledgerStat(quarantine);
    linkSync(quarantine, record.live);
    removeKnownFile(
      quarantine, captured, 'concurrent scaffold-state replacement', false,
      record.claim.directoryFd,
    );
    return true;
  } catch (error) {
    remember(errors, new Error(
      `preserved concurrent scaffold-state replacement at ${quarantine}: ${error.message}`,
    ));
    return false;
  }
}

function cleanupPrepared(record, preservePrior, errors) {
  if (preservePrior) {
    close(record.claim.fd, errors);
    close(record.prior.fd, errors);
    close(record.claim.directoryFd, errors);
    try { closeParentBoundary(record.boundary); } catch (error) { remember(errors, error); }
    return;
  }
  let artifactsRemoved = true;
  try {
    cleanupClaimArtifacts(record.claim, {
      priorIdentity: record.prior.identity,
      priorPaths: [record.priorQuarantine, record.priorSentinel],
      proof: record.proof,
      proofIdentity: record.unchanged ? record.prior.identity : record.claim.identity,
    });
  } catch (error) { remember(errors, error); artifactsRemoved = false; }
  close(record.claim.fd, errors);
  close(record.prior.fd, errors);
  close(record.claim.directoryFd, errors);
  if (artifactsRemoved) {
    try { removeClaimDirectory(record.claim); } catch (error) { remember(errors, error); }
  }
  try { closeParentBoundary(record.boundary); } catch (error) { remember(errors, error); }
}

function cancelLedgerPreparation(boundary, prior, claim, priorSentinel, cause) {
  const errors = [];
  let artifactsRemoved = false;
  if (claim !== null) {
    try {
      cleanupClaimArtifacts(claim, {
        priorIdentity: prior?.identity ?? null, priorPaths: [priorSentinel],
      });
      artifactsRemoved = true;
    } catch (error) { remember(errors, error); }
    close(claim.fd, errors);
    close(claim.directoryFd, errors);
    if (artifactsRemoved) {
      try { removeClaimDirectory(claim); } catch (error) { remember(errors, error); }
    }
  }
  if (prior !== null) close(prior.fd, errors);
  try { closeParentBoundary(boundary); } catch (error) { remember(errors, error); }
  if (errors.length > 0) throw new Error(
    `${cause.message}; ledger preparation cleanup failed: ${errors.map((e) => e.message).join('; ')}`,
  );
  throw cause;
}

export function prepareLedgerReservation(
  targetDir, path, content, stageName, expectedRoot, hooks = {}, authorize,
) {
  const bytes = Buffer.isBuffer(content) ? content : Buffer.from(content);
  const boundary = captureParentBoundary(
    targetDir, path, 'scaffold-state ledger', { expectedRoot },
  );
  let prior = null;
  let claim = null;
  let priorSentinel = null;
  try {
    const live = anchoredPath(boundary, basename(path));
    prior = openPriorLedger(live);
    claim = stagedLedger(boundary, bytes, prior.mode, stageName, hooks, authorize);
    writePriorClaim(claim, prior, hooks);
    priorSentinel = join(claim.directory, 'prior');
    linkSync(live, priorSentinel);
    requireLedgerStat(ledgerStat(priorSentinel), prior.identity, 2, prior.mode);
    requireLedgerStat(fstatSync(prior.fd, { bigint: true }), prior.identity, 2, prior.mode);
    return {
      boundary,
      claim,
      confirmed: false,
      content: bytes,
      live,
      prior,
      priorDetached: false,
      priorQuarantine: null,
      priorSentinel,
      replacementRecovery: null,
      proof: null,
      published: false,
      unchanged: prior.bytes.equals(bytes),
    };
  } catch (error) { cancelLedgerPreparation(boundary, prior, claim, priorSentinel, error); }
}

export function publishLedgerReservation(record) {
  assertParentBoundary(record.boundary);
  if (record.unchanged) return;
  record.priorQuarantine = randomStageArtifactPath(
    record.claim.directory, 'prior-live',
  );
  renameSync(record.live, record.priorQuarantine);
  record.priorDetached = true;
  const captured = ledgerStat(record.priorQuarantine);
  if (!sameIdentity(captured, record.prior.identity)) {
    const errors = [];
    if (restoreReplacement(record.priorQuarantine, record.live, errors)) {
      record.priorQuarantine = null;
    } else {
      record.replacementRecovery = record.priorQuarantine;
      record.priorQuarantine = null;
    }
    throw changedLedger(
      `pathname changed before publication${errors.length ? `; ${errors[0].message}` : ''}`,
    );
  }
  requireLedgerStat(captured, record.prior.identity, 2, record.prior.mode);
  linkSync(record.claim.sentinel, record.live);
  record.published = true;
  assertParentBoundary(record.boundary);
}

function assertLedgerInode(identity, fd, bytes, mode, links, paths) {
  const opened = requireLedgerStat(
    fstatSync(fd, { bigint: true }), identity, links, mode,
  );
  for (const path of paths) requireLedgerStat(ledgerStat(path), identity, links, mode);
  if (Number(opened.size) !== bytes.length || !descriptorMatches(fd, bytes)) {
    throw changedLedger('descriptor bytes differ');
  }
}

export function confirmLedgerReservation(record) {
  assertParentBoundary(record.boundary);
  record.proof = join(record.claim.directory, 'commit-proof');
  linkSync(record.live, record.proof);
  if (record.unchanged) {
    assertLedgerInode(record.prior.identity, record.prior.fd, record.prior.bytes,
      record.prior.mode, 3, [record.live, record.priorSentinel, record.proof]);
    assertLedgerInode(record.claim.identity, record.claim.fd, record.content,
      record.prior.mode, 1, [record.claim.sentinel]);
  } else {
    assertLedgerInode(record.claim.identity, record.claim.fd, record.content,
      record.prior.mode, 3, [record.claim.sentinel, record.live, record.proof]);
    assertLedgerInode(record.prior.identity, record.prior.fd, record.prior.bytes,
      record.prior.mode, 2, [record.priorSentinel, record.priorQuarantine]);
  }
  assertParentBoundary(record.boundary);
  record.confirmed = true;
}

function quarantineLive(record, errors) {
  const quarantine = randomStageArtifactPath(
    record.claim.directory, 'rollback-live',
  );
  try { renameSync(record.live, quarantine); } catch (error) {
    if (error?.code !== 'ENOENT') remember(errors, error);
    return true;
  }
  let stat;
  try { stat = ledgerStat(quarantine); } catch (error) {
    remember(errors, error);
    return false;
  }
  if (sameIdentity(stat, record.claim.identity)) {
    try {
      removeKnownFile(
        quarantine, record.claim.identity, 'scaffold-state rollback', false,
        record.claim.directoryFd,
      );
    } catch (error) { remember(errors, error); }
    return true;
  }
  restoreReplacement(record, quarantine, errors);
  return false;
}

export function rollbackLedgerReservation(record) {
  const errors = [];
  let liveVacant = true;
  if (record.published || record.priorDetached) liveVacant = quarantineLive(record, errors);
  if (record.priorDetached && liveVacant) {
    try { linkSync(record.priorSentinel, record.live); } catch (error) {
      liveVacant = false;
      remember(errors, new Error(
        `preserved prior scaffold-state at ${record.priorSentinel}: ${error.message}`,
      ));
    }
  }
  const preservePrior = record.priorDetached && !liveVacant;
  if (preservePrior) remember(errors, new Error(
    `preserved prior scaffold-state at ${record.priorSentinel}`,
  ));
  cleanupPrepared(record, preservePrior, errors);
  if (errors.length > 0) throw new Error(errors.map((error) => error.message).join('; '));
}

export function releaseLedgerReservation(record) {
  if (!record.confirmed) throw new Error('scaffold-state ledger was not confirmed');
  const errors = [];
  cleanupPrepared(record, false, errors);
  if (errors.length > 0) throw new Error(
    `scaffold-state committed but cleanup failed: ${errors.map((error) => error.message).join('; ')}`,
  );
}
