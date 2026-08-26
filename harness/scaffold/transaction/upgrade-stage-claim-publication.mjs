// Stage-intent authorization and durable publication of the self-bound claim.
// Filesystem shape validation and recovery cleanup remain with stage-claim.
import { createHash } from 'node:crypto';
import { fsyncSync } from 'node:fs';
import { join } from 'node:path';

export const STAGE_CLAIM = 'stage-claim.json';

export function claimDigest(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function modeOf(stat) {
  return Number(stat.mode & 0o777n);
}

function stageDocument(stageName, directory, next, bytes, control) {
  return {
    api_version: 'forgeos.scaffold-upgrade-stage-claim/v1',
    control: { dev: String(control.dev), ino: String(control.ino) },
    directory: { dev: String(directory.dev), ino: String(directory.ino) },
    next: {
      dev: String(next.dev), ino: String(next.ino), mode: modeOf(next),
      sha256: claimDigest(bytes),
    },
    stage_name: stageName,
  };
}

export function authorizeStage(authorize, directoryIdentity, mode, content, name, label) {
  if (typeof authorize !== 'function') throw new Error(`missing stage intent for ${label}`);
  authorize({
    directoryIdentity, nextMode: mode & 0o777,
    nextSha256: claimDigest(content), stageName: name,
  });
}

export function publishStageClaim(state, writeClaim) {
  const claimPath = join(state.descriptor, STAGE_CLAIM);
  const controlIdentity = writeClaim(
    claimPath,
    (control) => stageDocument(
      state.name, state.directory, state.next, state.content, control,
    ),
    `stage claim for ${state.label}`,
  );
  fsyncSync(state.directoryFd);
  fsyncSync(state.parentFd);
  return { claimPath, controlIdentity };
}
