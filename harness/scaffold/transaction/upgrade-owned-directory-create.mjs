// Identity-conditional creation cleanup for transaction-owned directories.
import {
  chmodSync, closeSync, constants, fchmodSync, fstatSync, fsyncSync, lstatSync,
  mkdirSync, openSync, writeFileSync,
} from 'node:fs';
import { join } from 'node:path';

import {
  removeKnownDirectory, removeKnownFile,
} from './upgrade-stage-claim.mjs';

const DIRECTORY = constants.O_DIRECTORY;
const NOFOLLOW = constants.O_NOFOLLOW;
const NONBLOCK = constants.O_NONBLOCK ?? 0;
const OWNER_MARKER = '.forge-upgrade-owner';

function identity(stat) { return Object.freeze({ dev: stat.dev, ino: stat.ino }); }
function sameIdentity(left, right) {
  return left.dev === right.dev && left.ino === right.ino;
}
function descriptorPath(root, fd, leaf = null) {
  const base = join(root, String(fd));
  return leaf === null ? base : join(base, leaf);
}
function syncDirectory(fd) { fsyncSync(fd); }

function writeOwnerMarker(path, bytes) {
  let fd;
  let created = null;
  try {
    fd = openSync(
      path, constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | NOFOLLOW | NONBLOCK,
      0o600,
    );
    created = identity(fstatSync(fd, { bigint: true }));
    writeFileSync(fd, bytes);
    fchmodSync(fd, 0o600);
    fsyncSync(fd);
    const opened = fstatSync(fd, { bigint: true });
    const lexical = lstatSync(path, { bigint: true });
    if (!opened.isFile() || Number(opened.nlink) !== 1
        || Number(opened.mode & 0o777n) !== 0o600
        || !sameIdentity(opened, lexical)) throw new Error('unsafe upgrade directory owner');
    return identity(opened);
  } catch (error) {
    if (created !== null) {
      try { removeKnownFile(path, created, 'failed upgrade directory owner', true); }
      catch (cleanup) { throw new Error(`${error.message}; ${cleanup.message}`); }
    }
    throw error;
  } finally { if (fd !== undefined) closeSync(fd); }
}

function cleanupFailedDirectory(state, cause) {
  const errors = [];
  if (state.markerIdentity !== null) {
    try {
      removeKnownFile(
        state.marker, state.markerIdentity, 'failed upgrade directory owner',
        true, state.directoryFd,
      );
    } catch (error) { errors.push(error); }
  }
  if (state.directoryIdentity !== null) {
    try {
      removeKnownDirectory(
        state.destination, state.directoryIdentity, 'failed owned upgrade directory',
        true, state.parentFd,
      );
    } catch (error) { errors.push(error); }
  }
  if (state.directoryFd !== undefined) {
    try { closeSync(state.directoryFd); } catch (error) { errors.push(error); }
  }
  if (errors.length > 0) throw new Error(
    `${cause.message}; directory cleanup failed: ${errors.map((item) => item.message).join('; ')}`,
  );
  throw cause;
}

export function createOwnedDirectoryComponent(parent, spec, label, markerBytes) {
  const destination = descriptorPath(parent.descriptorRoot, parent.fd, spec.name);
  try { mkdirSync(destination, { mode: 0o700 }); } catch (error) {
    if (error?.code === 'EEXIST') return false;
    throw error;
  }
  let created = null; let directoryFd; let markerIdentity = null;
  try {
    chmodSync(destination, 0o700);
    created = lstatSync(destination, { bigint: true });
    directoryFd = openSync(destination, constants.O_RDONLY | DIRECTORY | NOFOLLOW);
    const opened = fstatSync(directoryFd, { bigint: true });
    if (!opened.isDirectory() || !sameIdentity(opened, created)) {
      throw new Error('unsafe owned upgrade directory');
    }
    const marker = descriptorPath(parent.descriptorRoot, directoryFd, OWNER_MARKER);
    markerIdentity = writeOwnerMarker(marker, markerBytes(opened));
    syncDirectory(directoryFd); syncDirectory(parent.fd);
    closeSync(directoryFd); directoryFd = undefined;
    return true;
  } catch (error) {
    cleanupFailedDirectory({
      destination, directoryFd,
      directoryIdentity: created === null || directoryFd === undefined
        ? null : identity(created),
      marker: directoryFd === undefined ? null
        : descriptorPath(parent.descriptorRoot, directoryFd, OWNER_MARKER),
      markerIdentity, parentFd: parent.fd,
    }, new Error(`${error.message} (${label})`));
  }
}
