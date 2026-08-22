// Identity-conditional cleanup for one prepared upgrade record.
import { closeSync } from 'node:fs';
import {
  cleanupClaimArtifacts, removeClaimDirectory,
} from './upgrade-stage-cleanup.mjs';

function remember(errors, error) {
  errors.push(error instanceof Error ? error : new Error(String(error)));
}

function closeDescriptor(fd, errors) {
  if (fd === null || fd === undefined) return;
  try { closeSync(fd); } catch (error) { remember(errors, error); }
}

export function cleanupUpgradeRecord(record, preservePrior, errors, hooks = {}) {
  if (record.claim === null) {
    closeDescriptor(record.prior?.fd, errors);
    return;
  }
  if (preservePrior) {
    closeDescriptor(record.claim?.fd, errors);
    closeDescriptor(record.prior?.fd, errors);
    closeDescriptor(record.claim?.directoryFd, errors);
    return;
  }
  let artifactsRemoved = true;
  try {
    cleanupClaimArtifacts(record.claim, {
      priorIdentity: record.prior?.identity ?? null,
      priorPaths: [record.priorQuarantine, record.priorSentinel],
      proof: record.proof, proofIdentity: record.claim.identity,
    });
  } catch (error) { remember(errors, error); artifactsRemoved = false; }
  closeDescriptor(record.claim?.fd, errors);
  closeDescriptor(record.prior?.fd, errors);
  if (record.claim !== null) {
    closeDescriptor(record.claim.directoryFd, errors);
    if (artifactsRemoved) {
      try { removeClaimDirectory(record.claim, hooks); } catch (error) { remember(errors, error); }
    }
  }
}
