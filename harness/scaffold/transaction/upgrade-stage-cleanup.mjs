// Restart-safe stage cleanup. A self-bound stage claim is hard-linked beside
// the stage until the randomly detached directory has been removed.
import {
  closeSync, fsyncSync, linkSync, lstatSync,
} from 'node:fs';
import { basename, dirname, join } from 'node:path';

import {
  PRIOR_CLAIM, STAGE_CLAIM, findStageArtifact, readStageClaimPath,
  removalCandidates, removeKnownDirectory, removeKnownFile,
  validateClaimedFile,
} from './upgrade-stage-claim.mjs';

export function stageCleanupProofPath(parent, stageName) {
  return join(parent, `${stageName}.cleanup-proof`);
}

function sameIdentity(left, right) {
  return left.dev === right.dev && left.ino === right.ino;
}

function syncDirectory(fd) {
  fsyncSync(fd);
}

function artifactIdentity(path, label) {
  let canonical = null;
  try { canonical = lstatSync(path, { bigint: true }); } catch (error) {
    if (error?.code !== 'ENOENT') throw error;
  }
  const detached = removalCandidates(path);
  if (detached.length > 1 || (canonical !== null && detached.length > 0)) {
    throw new Error(`ambiguous detached ${label}`);
  }
  return canonical ?? (detached.length === 1
    ? lstatSync(detached[0], { bigint: true }) : null);
}

function expectedArtifact(stage, name, path) {
  if (name === 'next') return stage.nextClaim.identity;
  if (name === 'prior' || name.startsWith('prior-live')) {
    return stage.priorClaim?.identity ?? null;
  }
  if (name === PRIOR_CLAIM) return stage.priorClaim?.controlIdentity ?? null;
  const found = artifactIdentity(path, `upgrade stage ${name}`);
  if (found === null) return null;
  for (const expected of [stage.nextClaim.identity, stage.priorClaim?.identity]) {
    if (expected !== null && expected !== undefined && sameIdentity(found, expected)) {
      return expected;
    }
  }
  throw new Error(`preserved unknown upgrade stage ${name}`);
}

function validateArtifact(stage, name, path, expected) {
  if (name === 'next') {
    validateClaimedFile(path, stage.nextClaim, 'upgrade stage next');
  } else if ((name === 'prior' || name.startsWith('prior-live'))
      && stage.priorClaim !== null) {
    validateClaimedFile(path, stage.priorClaim, `upgrade stage ${name}`);
  } else if (name === 'commit-proof' || name.startsWith('rollback-live')) {
    const claim = sameIdentity(expected, stage.nextClaim.identity)
      ? stage.nextClaim : stage.priorClaim;
    validateClaimedFile(path, claim, `upgrade stage ${name}`);
  }
}

function cleanupStageFiles(stage) {
  const artifacts = [
    ...['commit-proof', 'prior', 'next', PRIOR_CLAIM].map(
      (name) => [name, join(stage.path, name)],
    ),
  ];
  for (const prefix of ['prior-live', 'rollback-live']) {
    const path = findStageArtifact(stage.path, prefix, `upgrade stage ${prefix}`);
    if (path !== null) artifacts.push([basename(path), path]);
  }
  for (const [name, path] of artifacts) {
    const expected = expectedArtifact(stage, name, path);
    if (expected === null) {
      if (artifactIdentity(path, `upgrade stage ${name}`) !== null) {
        throw new Error(`preserved unknown upgrade stage ${name}`);
      }
      continue;
    }
    validateArtifact(stage, name, path, expected);
    removeKnownFile(path, expected, `upgrade stage ${name}`, true, stage.fd);
  }
}

function ensureCleanupProof(stage) {
  let proof = readStageClaimPath(
    stage.proofPath, stage.stageName, stage.stat, true, stage.authority ?? null,
  );
  if (proof === null) {
    linkSync(join(stage.path, STAGE_CLAIM), stage.proofPath);
    syncDirectory(stage.parentFd);
    proof = readStageClaimPath(
      stage.proofPath, stage.stageName, stage.stat, false,
      stage.authority ?? null,
    );
  }
  if (!sameIdentity(proof.controlIdentity, stage.claim.controlIdentity)
      || !sameIdentity(proof.directoryIdentity, stage.claim.directoryIdentity)) {
    throw new Error('changed upgrade stage cleanup proof');
  }
  return proof;
}

function ensureClaimCleanupProof(claim) {
  const proofPath = stageCleanupProofPath(
    dirname(claim.publishedDirectory), claim.stageName,
  );
  let proof = readStageClaimPath(
    proofPath, claim.stageName, claim.directoryIdentity, true,
  );
  if (proof === null) {
    linkSync(claim.claimPath, proofPath);
    syncDirectory(claim.parentFd);
    proof = readStageClaimPath(proofPath, claim.stageName, claim.directoryIdentity);
  }
  if (!sameIdentity(proof.controlIdentity, claim.controlIdentity)) {
    throw new Error('changed upgrade stage cleanup proof');
  }
  claim.cleanupProof = { identity: proof.controlIdentity, path: proofPath };
}

export function cleanupClaimArtifacts(claim, options = {}) {
  const {
    priorIdentity = null, priorPaths = [], proof = null, proofIdentity = null,
  } = options;
  if (proof !== null) {
    removeKnownFile(proof, proofIdentity, 'upgrade stage commit proof', true, claim.directoryFd);
  }
  removeKnownFile(claim.stage, claim.identity, 'upgrade stage next', true, claim.directoryFd);
  for (const path of priorPaths) {
    if (path !== null) {
      removeKnownFile(path, priorIdentity, 'upgrade stage prior', true, claim.directoryFd);
    }
  }
  if (claim.priorClaimPath !== undefined) {
    removeKnownFile(
      claim.priorClaimPath, claim.priorControlIdentity,
      'upgrade prior claim', true, claim.directoryFd,
    );
  }
  ensureClaimCleanupProof(claim);
  removeKnownFile(
    claim.claimPath, claim.controlIdentity, 'upgrade stage claim', true, claim.directoryFd,
  );
}

export function removeClaimDirectory(claim, hooks = {}) {
  if (claim.cleanupProof === undefined) {
    throw new Error('missing upgrade stage cleanup proof');
  }
  removeKnownDirectory(
    claim.publishedDirectory, claim.directoryIdentity, 'upgrade stage directory',
    true, claim.parentFd, hooks.afterUpgradeStageDirectoryDetach,
  );
  removeKnownFile(
    claim.cleanupProof.path, claim.cleanupProof.identity, 'upgrade stage cleanup proof',
    false, claim.parentFd,
  );
}

export function cleanupClaimedStage(stage, hooks = {}) {
  hooks.beforeUpgradeStageFileCleanup?.({ path: stage.publishedPath });
  cleanupStageFiles(stage);
  const proof = ensureCleanupProof(stage);
  removeKnownFile(
    join(stage.path, STAGE_CLAIM), stage.claim.controlIdentity,
    'upgrade stage claim', true, stage.fd,
  );
  hooks.beforeUpgradeStageDirectoryDetach?.({ path: stage.publishedPath });
  try {
    removeKnownDirectory(
      stage.publishedPath, stage.claim.directoryIdentity, 'upgrade stage directory',
      false, stage.parentFd, hooks.afterUpgradeStageDirectoryDetach,
    );
  } finally {
    closeSync(stage.fd);
    stage.fd = null;
  }
  removeKnownFile(
    stage.proofPath, proof.controlIdentity, 'upgrade stage cleanup proof',
    false, stage.parentFd,
  );
}
