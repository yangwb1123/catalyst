// Descriptor-bound durable controls for a forge-upgrade transaction.
// Every operation stays below the retained target/.agent directory descriptor;
// lexical replacement is detected, while failure cleanup can still remove only
// controls whose inode identity was created by this process.
import {
  closeSync, constants, fstatSync, fsyncSync, lstatSync, openSync,
  linkSync, readFileSync, readdirSync, renameSync, unlinkSync,
} from 'node:fs';
import { randomBytes } from 'node:crypto';
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import {
  cleanupAuthorityDocument, completeControlPublication,
  decodeCleanupAuthority, publishControlFile,
} from './upgrade-transaction-authority.mjs';

const DIRECTORY = constants.O_DIRECTORY;
const NOFOLLOW = constants.O_NOFOLLOW;
const NONBLOCK = constants.O_NONBLOCK ?? 0;
const MAX_CONTROL_BYTES = 1024 * 1024;
const SUFFIXES = Object.freeze({
  cleanup: '.cleanup-proof', committed: '.committed', finished: '.finished',
  journal: '', prepared: '.prepared', started: '.started',
});
const CLEANED_CONTROLS = Object.freeze([
  'committed', 'prepared', 'started', 'finished',
]);

function sameIdentity(left, right) {
  return left.dev === right.dev && left.ino === right.ino;
}

function descriptorRoot() {
  for (const candidate of ['/proc/self/fd', '/dev/fd']) {
    try { if (lstatSync(candidate).isDirectory()) return candidate; } catch {}
  }
  throw new Error('forge-upgrade controls require descriptor-relative paths');
}

function directoryStat(path) {
  const stat = lstatSync(path, { bigint: true });
  if (stat.isSymbolicLink() || !stat.isDirectory()) {
    throw new Error(`unsafe scaffold upgrade control directory: ${path}`);
  }
  return stat;
}

function openDirectory(path, lexical, root) {
  let fd;
  try {
    fd = openSync(path, constants.O_RDONLY | DIRECTORY | NOFOLLOW);
    const opened = fstatSync(fd, { bigint: true });
    const anchored = directoryStat(path);
    const current = directoryStat(lexical);
    if (!opened.isDirectory() || !sameIdentity(opened, anchored)
        || !sameIdentity(opened, current)) throw new Error(`changed control directory: ${lexical}`);
    return { dev: opened.dev, fd, ino: opened.ino, path: lexical, root };
  } catch (error) {
    if (fd !== undefined) closeSync(fd);
    throw error;
  }
}

function captureControlBoundary(targetDir, destination) {
  const rootPath = resolve(targetDir);
  const parentPath = resolve(dirname(destination));
  const suffix = relative(rootPath, parentPath);
  if (suffix === '..' || suffix.startsWith(`..${sep}`) || isAbsolute(suffix)) {
    throw new Error('scaffold upgrade control directory escapes target');
  }
  const root = descriptorRoot();
  const components = [openDirectory(rootPath, rootPath, root)];
  try {
    let lexical = rootPath;
    for (const name of suffix.split(sep).filter(Boolean)) {
      lexical = join(lexical, name);
      const anchored = join(root, String(components.at(-1).fd), name);
      components.push(openDirectory(anchored, lexical, root));
    }
    return { components, root };
  } catch (error) {
    for (const item of components.reverse()) closeSync(item.fd);
    throw error;
  }
}

function assertControlBoundary(boundary) {
  for (const item of boundary.components) {
    const opened = fstatSync(item.fd, { bigint: true });
    const lexical = directoryStat(item.path);
    if (opened.dev !== item.dev || opened.ino !== item.ino || !sameIdentity(opened, lexical)) {
      throw new Error(`changed scaffold upgrade control directory: ${item.path}`);
    }
  }
}

function closeControlBoundary(boundary) {
  for (const item of [...boundary.components].reverse()) closeSync(item.fd);
}

function anchoredControl(boundary, leaf) {
  if (!leaf || leaf.includes('/') || leaf.includes('\\')) throw new Error('unsafe control leaf');
  return join(boundary.root, String(boundary.components.at(-1).fd), leaf);
}

function permissions(stat) { return Number(stat.mode & 0o777n); }

function controlError(label, detail = '') {
  return new Error(`unsafe ${label}${detail ? `: ${detail}` : ''}`);
}

function requireControl(stat, label, expected = null) {
  if (stat.isSymbolicLink() || !stat.isFile() || Number(stat.nlink) !== 1
      || permissions(stat) !== 0o600 || Number(stat.size) > MAX_CONTROL_BYTES
      || (expected !== null && !sameIdentity(stat, expected))) {
    throw controlError(label);
  }
  return stat;
}

function requireLinkedControl(stat, label, expected) {
  if (stat.isSymbolicLink() || !stat.isFile() || Number(stat.nlink) > 2
      || permissions(stat) !== 0o600 || Number(stat.size) > MAX_CONTROL_BYTES
      || !sameIdentity(stat, expected)) throw controlError(label);
  return stat;
}

function syncDirectory(control) {
  const fd = control.boundary.components.at(-1).fd;
  fsyncSync(fd);
}

function identity(stat) {
  return Object.freeze({ dev: stat.dev, ino: stat.ino });
}

function pathFor(control, name) {
  if (!Object.hasOwn(SUFFIXES, name)) throw new Error(`unknown upgrade control: ${name}`);
  return control.paths[name];
}

function removalCandidates(path) {
  const prefix = `${basename(path)}.removing-`;
  try {
    return readdirSync(dirname(path)).filter((name) => name.startsWith(prefix))
      .map((name) => join(dirname(path), name));
  } catch (error) {
    if (error?.code === 'ENOENT') return [];
    throw error;
  }
}

function selectedControlPath(path, label, optional) {
  let canonical = false;
  try { lstatSync(path); canonical = true; } catch (error) {
    if (error?.code !== 'ENOENT') throw error;
  }
  const detached = removalCandidates(path);
  if (detached.length > 1 || (canonical && detached.length > 0)) {
    throw controlError(label, 'ambiguous detached inode');
  }
  if (canonical) return path;
  if (detached.length === 1) return detached[0];
  return optional ? null : path;
}

export function openUpgradeControls(targetDir, relativePath) {
  const destination = join(targetDir, relativePath);
  const boundary = captureControlBoundary(targetDir, destination);
  const leaf = basename(destination);
  const paths = Object.fromEntries(Object.entries(SUFFIXES).map(
    ([name, suffix]) => [name, anchoredControl(boundary, `${leaf}${suffix}`)],
  ));
  const root = boundary.components[0];
  return {
    boundary, closed: false, identities: new Map(), paths,
    targetIdentity: Object.freeze({ dev: root.dev, ino: root.ino }),
  };
}

export function assertUpgradeControls(control) {
  assertControlBoundary(control.boundary);
}

export function writeUpgradeControl(
  control, name, bytes, detached = false, options = {},
) {
  if (!detached) assertUpgradeControls(control);
  const path = pathFor(control, name);
  const parentFd = control.boundary.components.at(-1).fd;
  const created = publishControlFile(path, name, bytes, parentFd, {
    ...options,
    onDurable(durableIdentity) {
      control.identities.set(name, durableIdentity);
      options.onDurable?.(durableIdentity);
    },
  });
  control.identities.set(name, created);
  if (!detached) assertUpgradeControls(control);
  return created;
}

export function readUpgradeControl(control, name, label, optional = false) {
  const path = pathFor(control, name);
  let fd;
  try {
    completeControlPublication(
      path, `${name} control`, control.boundary.components.at(-1).fd,
    );
    fd = openSync(path, constants.O_RDONLY | NOFOLLOW | NONBLOCK);
    const opened = requireControl(fstatSync(fd, { bigint: true }), label);
    const lexical = requireControl(lstatSync(path, { bigint: true }), label);
    if (!sameIdentity(opened, lexical)) throw controlError(label, 'identity changed');
    const result = { bytes: readFileSync(fd), identity: identity(opened) };
    control.identities.set(name, result.identity);
    return result;
  } catch (error) {
    if (optional && error?.code === 'ENOENT') return null;
    throw error;
  } finally {
    if (fd !== undefined) closeSync(fd);
  }
}

export function upgradeMarkerId(record, label) {
  if (record === null) return null;
  if (label === 'marker') {
    try {
      const value = JSON.parse(record.bytes);
      if (/^[a-f0-9]{32}$/.test(value?.transaction_id ?? '')) {
        return value.transaction_id;
      }
    } catch {}
  }
  const match = record.bytes.toString('utf8').match(/^([a-f0-9]{32})\n$/);
  if (match === null) throw new Error(`malformed scaffold upgrade transaction ${label}`);
  return match[1];
}

export function readUpgradeControlSet(control) {
  assertUpgradeControls(control);
  const state = {
    committed: readUpgradeControl(control, 'committed', 'upgrade commit marker', true),
    finished: readUpgradeControl(control, 'finished', 'upgrade finish marker', true),
    journal: readUpgradeControl(control, 'journal', 'upgrade transaction journal', true),
    prepared: readUpgradeControl(control, 'prepared', 'upgrade prepared authority', true),
    started: readUpgradeControl(control, 'started', 'upgrade transaction marker', true),
  };
  assertUpgradeControls(control);
  return state;
}

export function requireUpgradeMarkerIds(state, id) {
  for (const [name, label] of [
    ['started', 'marker'], ['committed', 'commit marker'], ['finished', 'finish marker'],
  ]) {
    const found = upgradeMarkerId(state[name], label);
    if (found !== null && found !== id) throw new Error('scaffold upgrade transaction marker mismatch');
  }
}

export function readUpgradeCleanupProof(control, authority = null, detached = false) {
  if (!detached) assertUpgradeControls(control);
  const path = pathFor(control, 'cleanup');
  completeControlPublication(
    path, 'cleanup control', control.boundary.components.at(-1).fd,
  );
  const selected = selectedControlPath(path, 'cleanup control', true);
  if (selected === null) return null;
  let fd;
  try {
    fd = openSync(selected, constants.O_RDONLY | NOFOLLOW | NONBLOCK);
    const opened = requireControl(fstatSync(fd, { bigint: true }), 'cleanup control');
    const lexical = requireControl(lstatSync(selected, { bigint: true }), 'cleanup control', opened);
    if (!sameIdentity(opened, lexical)) throw controlError('cleanup control', 'identity changed');
    const proof = decodeCleanupAuthority(
      readFileSync(fd), opened, control.targetIdentity, authority,
    );
    for (const [name, value] of Object.entries(proof.controls)) {
      if (value !== null) control.identities.set(name, value);
    }
    control.identities.set('cleanup', proof.identity);
    return proof;
  } finally { if (fd !== undefined) closeSync(fd); }
}

export function resumeUpgradeControlCleanup(control, proof, hooks = {}) {
  let failure = null;
  for (const name of CLEANED_CONTROLS) {
    try {
      removeUpgradeControl(control, name, {
        afterDetach: hooks.afterTransactionControlDetach,
        beforeDetach: hooks.beforeTransactionControlDetach,
        detached: true, expected: proof.controls[name],
      });
    } catch (error) { failure = error; break; }
  }
  if (failure !== null) throw failure;
  removeUpgradeControl(control, 'cleanup', {
    afterDetach: hooks.afterTransactionControlDetach,
    beforeDetach: hooks.beforeTransactionControlDetach,
    detached: true, expected: proof.identity, optional: false,
  });
  try {
    removeUpgradeControl(control, 'journal', {
      afterDetach: hooks.afterTransactionControlDetach,
      beforeDetach: hooks.beforeTransactionControlDetach,
      detached: true, expected: proof.controls.journal, optional: false,
    });
  } catch (error) {
    try {
      writeUpgradeControl(
        control, 'finished', Buffer.from(`${proof.transactionId}\n`), true,
      );
    } catch (markerError) {
      throw new Error(`${error.message}; finish evidence failed: ${markerError.message}`);
    }
    throw error;
  }
}

function finishDetachedControl(control, name, path, canonical, wanted) {
  let fd;
  try {
    fd = openSync(path, constants.O_RDONLY | NOFOLLOW | NONBLOCK);
    const opened = requireControl(
      fstatSync(fd, { bigint: true }), `${name} control`, wanted,
    );
    const lexical = requireControl(lstatSync(path, { bigint: true }), `${name} control`, opened);
    if (!sameIdentity(opened, lexical)) throw controlError(`${name} control`, 'identity changed');
    const quarantine = `${path}.finishing-${randomBytes(16).toString('hex')}`;
    renameSync(path, quarantine);
    const moved = requireControl(lstatSync(quarantine, { bigint: true }), `${name} control`);
    if (!sameIdentity(moved, wanted)) restoreDetachedControl(quarantine, path, name);
    unlinkSync(quarantine);
    try {
      lstatSync(canonical);
      throw controlError(`${name} control`, 'unknown canonical replacement preserved');
    } catch (error) {
      if (error?.code !== 'ENOENT') throw error;
    }
  } finally { if (fd !== undefined) closeSync(fd); }
  control.identities.delete(name);
  syncDirectory(control);
  return true;
}

function restoreDetachedControl(quarantine, path, name) {
  let fd;
  try {
    fd = openSync(quarantine, constants.O_RDONLY | NOFOLLOW | NONBLOCK);
    const opened = requireControl(fstatSync(fd, { bigint: true }), `${name} control`);
    const lexical = requireControl(lstatSync(quarantine, { bigint: true }), `${name} control`, opened);
    if (!sameIdentity(opened, lexical)) throw controlError(`${name} control`, 'identity changed');
    linkSync(quarantine, path);
    const finishing = `${quarantine}.restoring-${randomBytes(16).toString('hex')}`;
    renameSync(quarantine, finishing);
    const captured = requireLinkedControl(
      lstatSync(finishing, { bigint: true }), `${name} control`, opened,
    );
    if (!sameIdentity(captured, opened)) throw controlError(`${name} control`, 'replacement preserved');
    unlinkSync(finishing);
  } catch (error) {
    throw controlError(
      `${name} control`, `unknown inode preserved at ${quarantine}: ${error.message}`,
    );
  } finally { if (fd !== undefined) closeSync(fd); }
  throw controlError(`${name} control`, 'unknown replacement preserved');
}

export function removeUpgradeControl(
  control, name, {
    afterDetach = null, beforeDetach = null, detached = false,
    expected = undefined, optional = true,
  } = {},
) {
  if (!detached) assertUpgradeControls(control);
  const path = pathFor(control, name);
  const wanted = expected === undefined ? control.identities.get(name) ?? null : expected;
  const selected = selectedControlPath(path, `${name} control`, optional);
  if (selected === null) return false;
  if (wanted === null) throw controlError(`${name} control`, 'identity is unknown');
  if (selected !== path) return finishDetachedControl(control, name, selected, path, wanted);
  let fd;
  try {
    fd = openSync(path, constants.O_RDONLY | NOFOLLOW | NONBLOCK);
    requireControl(fstatSync(fd, { bigint: true }), `${name} control`, wanted);
    beforeDetach?.({ name, path });
    const quarantine = `${path}.removing-${randomBytes(16).toString('hex')}`;
    renameSync(path, quarantine);
    afterDetach?.({ name, path, quarantine });
    const moved = requireControl(lstatSync(quarantine, { bigint: true }), `${name} control`);
    if (!sameIdentity(moved, wanted)) restoreDetachedControl(quarantine, path, name);
    unlinkSync(quarantine);
  } catch (error) {
    if (optional && error?.code === 'ENOENT') return false;
    throw error;
  } finally { if (fd !== undefined) closeSync(fd); }
  control.identities.delete(name);
  syncDirectory(control);
  if (!detached) assertUpgradeControls(control);
  return true;
}

export function closeUpgradeControls(control) {
  if (control.closed) return;
  closeControlBoundary(control.boundary);
  control.closed = true;
}

export function finishUpgradeControls(control, transactionId, hooks = {}, detached = false) {
  const marker = Buffer.from(`${transactionId}\n`);
  let operationFailure = null;
  let cleanupFailure = null;
  try {
    if (!control.identities.has('finished')) {
      writeUpgradeControl(control, 'finished', marker, detached, { hooks });
      hooks.afterTransactionFinishMarker?.();
    }
  } catch (error) { operationFailure = error; }
  if (control.identities.has('finished')) {
    try {
      const authority = {
        controls: control.identities,
        journalIdentity: control.identities.get('journal'),
        transactionId,
      };
      let proof = readUpgradeCleanupProof(control, authority, detached);
      if (proof === null) {
        writeUpgradeControl(control, 'cleanup', (created) => Buffer.from(
          `${JSON.stringify(cleanupAuthorityDocument(
            control.identities, control.targetIdentity, transactionId, created,
          ))}\n`,
        ), detached, { hooks });
        proof = readUpgradeCleanupProof(control, authority, detached);
      }
      if (proof.transactionId !== transactionId) {
        throw controlError('cleanup control', 'transaction mismatch');
      }
      resumeUpgradeControlCleanup(control, proof, hooks);
    } catch (error) { cleanupFailure = error; }
  }
  try { closeUpgradeControls(control); } catch (error) {
    cleanupFailure ??= error;
  }
  const failure = operationFailure ?? cleanupFailure;
  if (failure !== null) {
    if (operationFailure !== null && cleanupFailure !== null) {
      throw new Error(`${operationFailure.message}; control cleanup failed: ${cleanupFailure.message}`);
    }
    throw failure;
  }
}

export function abandonUpgradeControls(control) {
  closeUpgradeControls(control);
}
