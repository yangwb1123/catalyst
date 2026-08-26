// Durable inode claims for transaction stage contents plus identity-conditional
// file cleanup. Recovery never removes a fixed stage name without these claims.
import {
  chmodSync, closeSync, constants, fchmodSync, fstatSync, fsyncSync, linkSync,
  lstatSync, mkdirSync, openSync, readFileSync, readdirSync, renameSync, rmdirSync,
  unlinkSync, writeFileSync,
} from 'node:fs';
import { randomBytes } from 'node:crypto';
import { basename, dirname, join } from 'node:path';

import {
  STAGE_CLAIM, authorizeStage, claimDigest as digest, publishStageClaim,
} from './upgrade-stage-claim-publication.mjs';

const DIRECTORY = constants.O_DIRECTORY;
const NOFOLLOW = constants.O_NOFOLLOW;
const NONBLOCK = constants.O_NONBLOCK ?? 0;
export { STAGE_CLAIM };
export const PRIOR_CLAIM = 'prior-claim.json';
const MAX_CLAIM_BYTES = 4096;
const RANDOM_ARTIFACT_SUFFIX = /^[a-f0-9]{32}$/;

function sameIdentity(left, right) {
  return left.dev === right.dev && left.ino === right.ino;
}

function identity(stat) { return Object.freeze({ dev: stat.dev, ino: stat.ino }); }
function modeOf(stat) { return Number(stat.mode & 0o777n); }
function descriptorPath(root, fd, leaf = null) {
  const base = join(root, String(fd));
  return leaf === null ? base : join(base, leaf);
}

export function randomStageArtifactPath(directory, prefix) {
  return join(directory, `${prefix}-${randomBytes(16).toString('hex')}`);
}

export function findStageArtifact(directory, prefix, label) {
  const names = readdirSync(directory).filter(
    (name) => name === prefix || name.startsWith(`${prefix}-`),
  );
  if (names.length > 1) throw new Error(`ambiguous ${label}`);
  if (names.length === 0) return null;
  const name = names[0];
  if (name !== prefix && !RANDOM_ARTIFACT_SUFFIX.test(name.slice(prefix.length + 1))) {
    throw new Error(`malformed ${label}`);
  }
  return join(directory, name);
}

function syncDirectory(fd) {
  fsyncSync(fd);
}

function requireRegular(stat, label, expected = null) {
  if (!stat.isFile() || stat.isSymbolicLink()
      || (expected !== null && !sameIdentity(stat, expected))) {
    throw new Error(`unsafe ${label}`);
  }
  return stat;
}

function writeClaim(path, document, label) {
  let fd;
  let createdIdentity = null;
  try {
    fd = openSync(
      path, constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | NOFOLLOW | NONBLOCK,
      0o600,
    );
    fchmodSync(fd, 0o600);
    const created = requireRegular(fstatSync(fd, { bigint: true }), label);
    createdIdentity = identity(created);
    const value = typeof document === 'function' ? document(identity(created)) : document;
    writeFileSync(fd, `${JSON.stringify(value)}\n`);
    fsyncSync(fd);
    const opened = requireRegular(fstatSync(fd, { bigint: true }), label);
    const lexical = requireRegular(lstatSync(path, { bigint: true }), label, opened);
    if (Number(opened.nlink) !== 1 || modeOf(opened) !== 0o600
        || !sameIdentity(opened, lexical)) throw new Error(`unsafe ${label}`);
    return identity(opened);
  } catch (error) {
    if (createdIdentity !== null) {
      try { removeKnownFile(path, createdIdentity, label, true); } catch (cleanup) {
        throw new Error(`${error.message}; claim cleanup failed: ${cleanup.message}`);
      }
    }
    throw error;
  } finally {
    if (fd !== undefined) closeSync(fd);
  }
}

function cleanupFailedClaim(state, cause) {
  const errors = [];
  const remember = (error) => errors.push(error instanceof Error ? error : new Error(String(error)));
  if (state.controlIdentity !== null) {
    try {
      removeKnownFile(
        state.claimPath, state.controlIdentity, 'failed upgrade stage claim', true,
        state.directoryFd,
      );
    } catch (error) { remember(error); }
  }
  if (state.nextIdentity !== null) {
    try {
      removeKnownFile(
        state.sentinel, state.nextIdentity, 'failed upgrade stage next', true,
        state.directoryFd,
      );
    } catch (error) { remember(error); }
  }
  if (state.directoryIdentity !== null) {
    try {
      removeKnownDirectory(
        state.directory, state.directoryIdentity, 'failed upgrade stage directory',
        true, state.parentFd,
      );
    } catch (error) { remember(error); }
  }
  for (const fd of [state.fd, state.directoryFd]) {
    if (fd !== undefined) try { closeSync(fd); } catch (error) { remember(error); }
  }
  if (errors.length > 0) throw new Error(
    `${cause.message}; claim cleanup failed: ${errors.map((error) => error.message).join('; ')}`,
  );
  throw cause;
}

function claimedPath(path, label, optional) {
  let canonical = false;
  try { lstatSync(path); canonical = true; } catch (error) {
    if (error?.code !== 'ENOENT') throw error;
  }
  const detached = removalCandidates(path);
  if (detached.length > 1 || (canonical && detached.length > 0)) {
    throw new Error(`ambiguous detached ${label}`);
  }
  if (canonical) return path;
  if (detached.length === 1) return detached[0];
  if (optional) return null;
  return path;
}

function readClaim(path, label, optional = false, maxLinks = 1) {
  let fd;
  try {
    const selected = claimedPath(path, label, optional);
    if (selected === null) return null;
    fd = openSync(selected, constants.O_RDONLY | NOFOLLOW | NONBLOCK);
    const opened = requireRegular(fstatSync(fd, { bigint: true }), label);
    const lexical = requireRegular(lstatSync(selected, { bigint: true }), label, opened);
    if (!sameIdentity(opened, lexical) || Number(opened.nlink) < 1
        || Number(opened.nlink) > maxLinks
        || modeOf(opened) !== 0o600 || Number(opened.size) > MAX_CLAIM_BYTES) {
      throw new Error(`unsafe ${label}`);
    }
    const bytes = readFileSync(fd);
    let document;
    try { document = JSON.parse(bytes); } catch { throw new Error(`malformed ${label}`); }
    return { document, identity: identity(opened) };
  } catch (error) {
    if (optional && error?.code === 'ENOENT') return null;
    throw error;
  } finally {
    if (fd !== undefined) closeSync(fd);
  }
}

function decimal(value, positive = false) {
  return typeof value === 'string'
    && (positive ? /^[1-9]\d*$/.test(value) : /^(0|[1-9]\d*)$/.test(value));
}

function exactKeys(value, expected) {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    && JSON.stringify(Object.keys(value).sort()) === JSON.stringify(expected);
}

function assertStageAuthority(claim, authority) {
  if (authority === null) return claim;
  if (!sameIdentity(claim.controlIdentity, authority.controlIdentity)
      || !sameIdentity(claim.directoryIdentity, authority.directoryIdentity)
      || !sameIdentity(claim.nextIdentity, authority.nextIdentity)
      || claim.nextMode !== authority.nextMode
      || claim.nextSha256 !== authority.nextSha256) {
    throw new Error('changed scaffold upgrade stage authority');
  }
  return claim;
}

function decodeStageClaim(record, stageName, directory = null, authority = null) {
  const value = record.document;
  if (!exactKeys(value, ['api_version', 'control', 'directory', 'next', 'stage_name'])
      || value.api_version !== 'forgeos.scaffold-upgrade-stage-claim/v1'
      || value.stage_name !== stageName
      || !exactKeys(value.control, ['dev', 'ino'])
      || value.control.dev !== String(record.identity.dev)
      || value.control.ino !== String(record.identity.ino)
      || !exactKeys(value.directory, ['dev', 'ino'])
      || !decimal(value.directory.dev) || !decimal(value.directory.ino, true)
      || (directory !== null && (value.directory.dev !== String(directory.dev)
        || value.directory.ino !== String(directory.ino)))
      || !exactKeys(value.next, ['dev', 'ino', 'mode', 'sha256'])
      || !decimal(value.next.dev) || !decimal(value.next.ino, true)
      || !Number.isInteger(value.next.mode) || value.next.mode < 0 || value.next.mode > 0o777
      || !/^[a-f0-9]{64}$/.test(value.next.sha256 ?? '')) {
    throw new Error('malformed scaffold upgrade stage claim');
  }
  return assertStageAuthority({
    controlIdentity: record.identity,
    directoryIdentity: Object.freeze({
      dev: BigInt(value.directory.dev), ino: BigInt(value.directory.ino),
    }),
    nextIdentity: Object.freeze({ dev: BigInt(value.next.dev), ino: BigInt(value.next.ino) }),
    nextMode: value.next.mode,
    nextSha256: value.next.sha256,
  }, authority);
}

export function createTransactionClaim(
  boundary, name, content, mode, label, hooks = {}, authorize,
) {
  const parent = boundary.components.at(-1);
  const directory = descriptorPath(boundary.descriptorRoot, parent.fd, name);
  mkdirSync(directory, { mode: 0o700 });
  chmodSync(directory, 0o700);
  let directoryFd; let fd;
  let controlIdentity = null; let directoryIdentity = null; let nextIdentity = null;
  let claimPath = null; let sentinel = null;
  try {
    const created = lstatSync(directory, { bigint: true });
    directoryFd = openSync(directory, constants.O_RDONLY | DIRECTORY | NOFOLLOW);
    const openedDirectory = fstatSync(directoryFd, { bigint: true });
    directoryIdentity = identity(openedDirectory);
    if (!created.isDirectory() || !sameIdentity(created, openedDirectory)) {
      throw new Error(`unsafe claim directory for ${label}`);
    }
    authorizeStage(authorize, directoryIdentity, mode, content, name, label);
    hooks.afterUpgradeStageDirectoryCreate?.({ label, path: directory });
    const descriptor = descriptorPath(boundary.descriptorRoot, directoryFd);
    sentinel = join(descriptor, 'next');
    fd = openSync(
      sentinel, constants.O_RDWR | constants.O_CREAT | constants.O_EXCL | NOFOLLOW | NONBLOCK,
      mode,
    );
    fchmodSync(fd, mode & 0o777);
    nextIdentity = identity(requireRegular(fstatSync(fd, { bigint: true }), `claim for ${label}`));
    writeFileSync(fd, content);
    fsyncSync(fd);
    hooks.afterUpgradeStageNextSync?.({ label, path: sentinel });
    const next = requireRegular(fstatSync(fd, { bigint: true }), `claim for ${label}`);
    if (Number(next.nlink) !== 1) throw new Error(`unsafe claim for ${label}`);
    ({ claimPath, controlIdentity } = publishStageClaim({
      content, descriptor, directory: openedDirectory, directoryFd,
      label, name, next, parentFd: parent.fd,
    }, writeClaim));
    return {
      claimPath, controlIdentity, directory: descriptor,
      directoryFd, directoryIdentity, fd,
      identity: nextIdentity, parentFd: parent.fd, publishedDirectory: directory,
      nextMode: modeOf(next), nextSha256: digest(content),
      sentinel, stage: sentinel, stageName: name,
    };
  } catch (error) { cleanupFailedClaim({
    claimPath, controlIdentity, directory, directoryFd,
    directoryIdentity: directoryFd === undefined ? null : directoryIdentity,
    fd, nextIdentity, parentFd: parent.fd, sentinel,
  }, error); }
}

export function readStageClaimPath(
  path, stageName, directory = null, optional = false, authority = null,
) {
  const record = readClaim(path, 'upgrade stage claim', optional, 2);
  return record === null
    ? null : decodeStageClaim(record, stageName, directory, authority);
}

export function readStageClaim(stage, stageName, fallback = null, authority = null) {
  let claim = readStageClaimPath(
    join(stage.path, STAGE_CLAIM), stageName, stage.stat, true, authority,
  );
  if (claim === null && fallback !== null) {
    claim = readStageClaimPath(fallback, stageName, stage.stat, false, authority);
  }
  if (claim === null) throw new Error('missing upgrade stage claim');
  return claim;
}

function priorDocument(prior) {
  return {
    api_version: 'forgeos.scaffold-upgrade-prior-claim/v1',
    dev: String(prior.identity.dev), ino: String(prior.identity.ino),
    mode: prior.mode, sha256: digest(prior.bytes),
  };
}

export function writePriorClaim(claim, prior, hooks = {}) {
  const path = join(claim.directory, PRIOR_CLAIM);
  const priorControlIdentity = writeClaim(
    path, priorDocument(prior), 'upgrade prior claim',
  );
  syncDirectory(claim.directoryFd);
  Object.assign(claim, {
    priorClaimPath: path, priorControlIdentity,
    priorIdentity: prior.identity, priorMode: prior.mode,
    priorSha256: digest(prior.bytes),
  });
  hooks.afterUpgradePriorClaimSync?.({ path });
}

export function readPriorClaim(stage, authority = null) {
  const record = readClaim(join(stage.path, PRIOR_CLAIM), 'upgrade prior claim', true);
  if (record === null) return null;
  const value = record.document;
  if (!exactKeys(value, ['api_version', 'dev', 'ino', 'mode', 'sha256'])
      || value.api_version !== 'forgeos.scaffold-upgrade-prior-claim/v1'
      || !decimal(value.dev) || !decimal(value.ino, true)
      || !Number.isInteger(value.mode) || value.mode < 0 || value.mode > 0o777
      || !/^[a-f0-9]{64}$/.test(value.sha256 ?? '')) {
    throw new Error('malformed scaffold upgrade prior claim');
  }
  const claim = {
    controlIdentity: record.identity,
    identity: Object.freeze({ dev: BigInt(value.dev), ino: BigInt(value.ino) }),
    mode: value.mode, sha256: value.sha256,
  };
  if (authority !== null && (!sameIdentity(claim.controlIdentity, authority.controlIdentity)
      || !sameIdentity(claim.identity, authority.identity)
      || claim.mode !== authority.mode || claim.sha256 !== authority.sha256)) {
    throw new Error('changed scaffold upgrade prior authority');
  }
  return claim;
}

export function validateClaimedFile(path, expected, label, optional = true) {
  let fd;
  try {
    const selected = claimedPath(path, label, optional);
    if (selected === null) return null;
    fd = openSync(selected, constants.O_RDONLY | NOFOLLOW | NONBLOCK);
    const opened = requireRegular(fstatSync(fd, { bigint: true }), label,
      expected.identity ?? null);
    const lexical = requireRegular(lstatSync(selected, { bigint: true }), label, opened);
    if (!sameIdentity(opened, lexical) || modeOf(opened) !== expected.mode
        || digest(readFileSync(fd)) !== expected.sha256) throw new Error(`changed ${label}`);
    return identity(opened);
  } catch (error) {
    if (optional && error?.code === 'ENOENT') return null;
    throw error;
  } finally {
    if (fd !== undefined) closeSync(fd);
  }
}

function restoreMovedFile(quarantine, path, label) {
  let fd;
  try {
    fd = openSync(quarantine, constants.O_RDONLY | NOFOLLOW | NONBLOCK);
    const opened = requireRegular(fstatSync(fd, { bigint: true }), label);
    const lexical = requireRegular(lstatSync(quarantine, { bigint: true }), label, opened);
    if (!sameIdentity(opened, lexical)) throw new Error(`changed ${label}`);
    linkSync(quarantine, path);
    const restored = requireRegular(lstatSync(path, { bigint: true }), label, opened);
    if (!sameIdentity(restored, opened)) throw new Error(`changed restored ${label}`);
    const finishing = `${quarantine}.restoring-${randomBytes(16).toString('hex')}`;
    renameSync(quarantine, finishing);
    const captured = requireRegular(lstatSync(finishing, { bigint: true }), label);
    if (!sameIdentity(captured, opened)) throw new Error(`preserved replacement at ${finishing}`);
    unlinkSync(finishing);
  } catch (error) {
    throw new Error(`preserved unknown ${label} at ${quarantine}: ${error.message}`);
  } finally { if (fd !== undefined) closeSync(fd); }
}

export function removeKnownFile(path, expected, label, optional = true, directoryFd = null) {
  const detached = removalCandidates(path);
  if (detached.length > 1) throw new Error(`ambiguous detached ${label}`);
  if (detached.length === 1) {
    let canonical = null;
    try { canonical = lstatSync(path, { bigint: true }); } catch (error) {
      if (error?.code !== 'ENOENT') throw error;
    }
    if (canonical !== null) throw new Error(`ambiguous live and detached ${label}`);
    let fd;
    try {
      fd = openSync(detached[0], constants.O_RDONLY | NOFOLLOW | NONBLOCK);
      const moved = requireRegular(fstatSync(fd, { bigint: true }), label, expected);
      const lexical = requireRegular(lstatSync(detached[0], { bigint: true }), label, moved);
      if (!sameIdentity(moved, lexical)) throw new Error(`preserved unknown ${label}`);
      const finishing = `${detached[0]}.finishing-${randomBytes(16).toString('hex')}`;
      renameSync(detached[0], finishing);
      const captured = requireRegular(lstatSync(finishing, { bigint: true }), label);
      if (!sameIdentity(captured, expected)) restoreMovedFile(finishing, detached[0], label);
      unlinkSync(finishing);
    } finally { if (fd !== undefined) closeSync(fd); }
    if (directoryFd !== null) syncDirectory(directoryFd);
    return true;
  }
  const quarantine = `${path}.removing-${randomBytes(16).toString('hex')}`;
  let fd;
  try {
    fd = openSync(path, constants.O_RDONLY | NOFOLLOW | NONBLOCK);
    requireRegular(fstatSync(fd, { bigint: true }), label, expected);
    renameSync(path, quarantine);
    const moved = lstatSync(quarantine, { bigint: true });
    if (!sameIdentity(moved, expected)) {
      restoreMovedFile(quarantine, path, label);
      throw new Error(`refusing to remove unknown ${label}`);
    }
    unlinkSync(quarantine);
  } catch (error) {
    if (optional && error?.code === 'ENOENT') return false;
    throw error;
  } finally {
    if (fd !== undefined) closeSync(fd);
  }
  if (directoryFd !== null) syncDirectory(directoryFd);
  return true;
}

export function removalCandidates(path) {
  const prefix = `${basename(path)}.removing-`;
  try {
    return readdirSync(dirname(path)).filter((name) => name.startsWith(prefix))
      .map((name) => join(dirname(path), name));
  } catch (error) {
    if (error?.code === 'ENOENT') return [];
    throw error;
  }
}

function finishDetachedDirectory(path, expected, label, directoryFd) {
  let fd;
  try {
    fd = openSync(path, constants.O_RDONLY | DIRECTORY | NOFOLLOW);
    const opened = fstatSync(fd, { bigint: true });
    const lexical = lstatSync(path, { bigint: true });
    if (!opened.isDirectory() || !sameIdentity(opened, expected)
        || !sameIdentity(opened, lexical)) {
      throw new Error(`preserved unknown ${label} at ${path}`);
    }
    const finishing = `${path}.finishing-${randomBytes(16).toString('hex')}`;
    renameSync(path, finishing);
    const captured = lstatSync(finishing, { bigint: true });
    if (!captured.isDirectory() || !sameIdentity(captured, expected)) {
      throw new Error(`preserved unknown ${label} at ${finishing}`);
    }
    rmdirSync(finishing);
  } finally { if (fd !== undefined) closeSync(fd); }
  if (directoryFd !== null) syncDirectory(directoryFd);
}

export function removeKnownDirectory(
  path, expected, label, optional = true, directoryFd = null, afterDetach = null,
) {
  const detached = removalCandidates(path);
  if (detached.length > 1) throw new Error(`ambiguous detached ${label}`);
  if (detached.length === 1) {
    finishDetachedDirectory(detached[0], expected, label, directoryFd);
    return true;
  }
  const quarantine = `${path}.removing-${randomBytes(16).toString('hex')}`;
  let fd;
  try {
    fd = openSync(path, constants.O_RDONLY | DIRECTORY | NOFOLLOW);
    const opened = fstatSync(fd, { bigint: true });
    if (!opened.isDirectory() || !sameIdentity(opened, expected)) {
      throw new Error(`preserved unknown ${label} at ${path}`);
    }
    renameSync(path, quarantine);
    afterDetach?.({ path, quarantine });
    finishDetachedDirectory(quarantine, expected, label, directoryFd);
  } catch (error) {
    if (optional && error?.code === 'ENOENT') return false;
    throw error;
  } finally {
    if (fd !== undefined) closeSync(fd);
  }
  return true;
}
