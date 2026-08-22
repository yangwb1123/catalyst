// Opens a canonical or crash-detached stage through a pinned parent and binds
// it to the self-authenticated claim retained outside the directory.
import {
  closeSync, constants, fstatSync, lstatSync, openSync, readdirSync,
} from 'node:fs';
import { basename, join } from 'node:path';

import {
  STAGE_CLAIM, readPriorClaim, readStageClaim, readStageClaimPath, removalCandidates,
  removeKnownDirectory, removeKnownFile, validateClaimedFile,
} from './upgrade-stage-claim.mjs';
import { cleanupClaimedStage, stageCleanupProofPath } from './upgrade-stage-cleanup.mjs';

const DIRECTORY = constants.O_DIRECTORY;
const NOFOLLOW = constants.O_NOFOLLOW;

function sameIdentity(left, right) {
  return left.dev === right.dev && left.ino === right.ino;
}

function optionalStat(path) {
  try { return lstatSync(path, { bigint: true }); } catch (error) {
    if (error?.code === 'ENOENT') return null;
    throw error;
  }
}

function anchoredStagePath(boundary, name) {
  if (!name || name.includes('/') || name.includes('\\')) {
    throw new Error(`unsafe recovery stage name: ${name}`);
  }
  return join(
    boundary.descriptorRoot, String(boundary.components.at(-1).fd), name,
  );
}

function recoveryStagePath(path, rel) {
  const canonical = optionalStat(path);
  const detached = removalCandidates(path);
  if (detached.length > 1 || (canonical !== null && detached.length > 0)) {
    throw new Error(`ambiguous recovery stage for ${rel}`);
  }
  if (canonical !== null) return { path, stat: canonical };
  if (detached.length === 1) {
    return { path: detached[0], stat: lstatSync(detached[0], { bigint: true }) };
  }
  return null;
}

function recoveryLocation(boundary, entry) {
  const parentFd = boundary.components.at(-1).fd;
  const parentPath = join(boundary.descriptorRoot, String(parentFd));
  return {
    parentFd, parentPath,
    proofPath: stageCleanupProofPath(parentPath, entry.stage_name),
    publishedPath: anchoredStagePath(boundary, entry.stage_name),
  };
}

function openRecoveryDirectory(boundary, entry, expected = null) {
  const location = recoveryLocation(boundary, entry);
  const selected = recoveryStagePath(location.publishedPath, entry.rel);
  if (selected === null) return { ...location, stage: null };
  if (selected.stat.isSymbolicLink() || !selected.stat.isDirectory()) {
    throw new Error(`unsafe recovery stage for ${entry.rel}`);
  }
  if (expected !== null && !sameIdentity(selected.stat, expected)) {
    throw new Error(`changed recovery stage authority for ${entry.rel}`);
  }
  let fd;
  try {
    fd = openSync(selected.path, constants.O_RDONLY | DIRECTORY | NOFOLLOW);
    const opened = fstatSync(fd, { bigint: true });
    if (!sameIdentity(opened, selected.stat)) {
      throw new Error(`changed recovery stage for ${entry.rel}`);
    }
    return { ...location, stage: {
      fd, parentFd: location.parentFd, parentPath: location.parentPath,
      path: join(boundary.descriptorRoot, String(fd)), proofPath: location.proofPath,
      publishedPath: location.publishedPath, stageName: entry.stage_name, stat: opened,
    } };
  } catch (error) {
    if (fd !== undefined) closeSync(fd);
    throw error;
  }
}

function cleanupFinishedProof(parentFd, entry, proofPath, authority) {
  const proof = readStageClaimPath(
    proofPath, entry.stage_name, null, true, authority,
  );
  if (proof === null) return null;
  removeKnownFile(
    proofPath, proof.controlIdentity, 'upgrade stage cleanup proof', false,
    parentFd,
  );
  return null;
}

function populateClaims(stage, entry, authority) {
  stage.claim = readStageClaim(
    stage, entry.stage_name, stage.proofPath, authority,
  );
  stage.nextClaim = {
    identity: stage.claim.nextIdentity, mode: stage.claim.nextMode,
    sha256: stage.claim.nextSha256,
  };
  validateClaimedFile(join(stage.path, 'next'), stage.nextClaim, 'upgrade stage next');
  stage.priorClaim = readPriorClaim(stage, authority?.prior ?? null);
  if (authority !== null && (stage.priorClaim === null) !== (authority.prior === null)) {
    throw new Error(`changed prior authority for ${entry.rel}`);
  }
  const prior = optionalStat(join(stage.path, 'prior'));
  if (prior !== null && stage.priorClaim === null) {
    throw new Error(`missing prior claim for ${entry.rel}`);
  }
  if (stage.priorClaim !== null) {
    validateClaimedFile(join(stage.path, 'prior'), stage.priorClaim, 'upgrade stage prior');
  }
}

export function openUpgradeRecoveryStage(boundary, entry, authority) {
  const opened = openRecoveryDirectory(
    boundary, entry, authority?.directoryIdentity ?? null,
  );
  if (opened.stage === null) {
    return cleanupFinishedProof(opened.parentFd, entry, opened.proofPath, authority);
  }
  const stage = { ...opened.stage, authority };
  try { populateClaims(stage, entry, authority); return stage; } catch (error) {
    closeSync(stage.fd);
    throw error;
  }
}

function requireIntentClaim(claim, intent, entry) {
  if (!sameIdentity(claim.directoryIdentity, intent.directoryIdentity)
      || claim.nextMode !== intent.nextMode || claim.nextSha256 !== intent.nextSha256) {
    throw new Error(`changed unstarted stage intent authority for ${entry.rel}`);
  }
  return claim;
}

function intentStageClaim(stage, entry, intent) {
  let claim = readStageClaimPath(
    join(stage.path, STAGE_CLAIM), entry.stage_name, stage.stat, true,
  );
  if (claim === null) {
    claim = readStageClaimPath(
      stage.proofPath, entry.stage_name, stage.stat, true,
    );
  }
  return claim === null ? null : requireIntentClaim(claim, intent, entry);
}

function artifactExists(path) {
  return optionalStat(path) !== null || removalCandidates(path).length !== 0;
}

function populateIntentClaims(stage, entry, intent) {
  stage.claim = intentStageClaim(stage, entry, intent);
  if (stage.claim === null) return false;
  stage.nextClaim = {
    identity: stage.claim.nextIdentity, mode: stage.claim.nextMode,
    sha256: stage.claim.nextSha256,
  };
  validateClaimedFile(join(stage.path, 'next'), stage.nextClaim, 'upgrade stage next');
  stage.priorClaim = readPriorClaim(stage);
  const priorPath = join(stage.path, 'prior');
  if (stage.priorClaim === null && artifactExists(priorPath)) {
    throw new Error(`missing prior claim for ${entry.rel}`);
  }
  if (stage.priorClaim !== null) {
    const live = join(stage.parentPath, basename(entry.rel));
    validateClaimedFile(live, stage.priorClaim, `unstarted live prior ${entry.rel}`, false);
    validateClaimedFile(priorPath, stage.priorClaim, 'upgrade stage prior');
  }
  return true;
}

function cleanupIntentProof(opened, entry, intent) {
  const proof = readStageClaimPath(
    opened.proofPath, entry.stage_name, intent.directoryIdentity, true,
  );
  if (proof === null) return;
  requireIntentClaim(proof, intent, entry);
  removeKnownFile(
    opened.proofPath, proof.controlIdentity, 'upgrade stage cleanup proof', false,
    opened.parentFd,
  );
}

function cleanupIntentOnlyStage(boundary, entry, intent, stage, hooks) {
  hooks.beforeUpgradeStageFileCleanup?.({ path: stage.publishedPath });
  const nextPath = join(stage.path, 'next');
  const next = validateClaimedFile(nextPath, {
    mode: intent.nextMode, sha256: intent.nextSha256,
  }, 'unclaimed upgrade stage next');
  if (next !== null) {
    removeKnownFile(nextPath, next, 'unclaimed upgrade stage next', false, stage.fd);
  }
  closeSync(stage.fd);
  stage.fd = null;
  cleanupEmptyStage(boundary, entry, hooks, intent.directoryIdentity);
}

function cleanupIntentStage(boundary, entry, intent, hooks) {
  const opened = openRecoveryDirectory(boundary, entry, intent.directoryIdentity);
  if (opened.stage === null) return cleanupIntentProof(opened, entry, intent);
  const stage = { ...opened.stage, authority: null };
  try {
    if (populateIntentClaims(stage, entry, intent)) cleanupClaimedStage(stage, hooks);
    else cleanupIntentOnlyStage(boundary, entry, intent, stage, hooks);
  } finally {
    if (stage.fd !== null && stage.fd !== undefined) closeSync(stage.fd);
  }
}

function cleanupEmptyStage(boundary, entry, hooks, expected = null) {
  const publishedPath = anchoredStagePath(boundary, entry.stage_name);
  const selected = recoveryStagePath(publishedPath, entry.rel);
  if (selected === null) return;
  if (selected.stat.isSymbolicLink() || !selected.stat.isDirectory()) {
    throw new Error(`unsafe unstarted recovery stage for ${entry.rel}`);
  }
  let fd; let opened;
  try {
    fd = openSync(selected.path, constants.O_RDONLY | DIRECTORY | NOFOLLOW);
    opened = fstatSync(fd, { bigint: true });
    if (!sameIdentity(opened, selected.stat)
        || (expected !== null && !sameIdentity(opened, expected))) {
      throw new Error(`changed unstarted recovery stage for ${entry.rel}`);
    }
    if (readdirSync(join(boundary.descriptorRoot, String(fd))).length !== 0) {
      throw new Error(`preserved unclaimed upgrade stage artifacts for ${entry.rel}`);
    }
  } finally { if (fd !== undefined) closeSync(fd); }
  hooks.beforeUpgradeStageDirectoryDetach?.({ path: publishedPath });
  removeKnownDirectory(
    publishedPath, opened, `unstarted upgrade stage ${entry.rel}`, false,
    boundary.components.at(-1).fd, hooks.afterUpgradeStageDirectoryDetach,
  );
}

export function cleanupUnstartedUpgradeStage(boundary, entry, authority, intent, hooks) {
  if (authority === null && intent !== null) {
    cleanupIntentStage(boundary, entry, intent, hooks);
    return;
  }
  if (authority === null) {
    const location = recoveryLocation(boundary, entry);
    if (artifactExists(location.proofPath) || artifactExists(location.publishedPath)) {
      throw new Error(`preserved unclaimed upgrade stage artifacts for ${entry.rel}`);
    }
    return;
  }
  let stage;
  try {
    stage = openUpgradeRecoveryStage(boundary, entry, authority);
    if (stage !== null) cleanupClaimedStage(stage, hooks);
  } finally {
    if (stage?.fd !== null && stage?.fd !== undefined) closeSync(stage.fd);
  }
}
