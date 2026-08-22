// Descriptor-pinned path boundaries for forge-upgrade transactions.
// Missing components are created below an already-open parent and carry a
// transaction marker, so rollback removes only directories this transaction owns.
import {
  closeSync, constants, fstatSync, fsyncSync, linkSync, lstatSync,
  openSync, readFileSync, readdirSync,
} from 'node:fs';
import {
  basename, dirname, isAbsolute, join, relative, resolve, sep,
} from 'node:path';
import {
  removalCandidates, removeKnownDirectory, removeKnownFile,
} from './transaction/upgrade-stage-claim.mjs';
import { ownedDirectoryCleanupContext } from './transaction/upgrade-owned-directory-state.mjs';
import { createOwnedDirectoryComponent } from './transaction/upgrade-owned-directory-create.mjs';
import { cleanupUnstartedUpgradeStage } from './transaction/upgrade-stage-recovery.mjs';
const DIRECTORY = constants.O_DIRECTORY;
const NOFOLLOW = constants.O_NOFOLLOW;
const NONBLOCK = constants.O_NONBLOCK ?? 0;
const OWNER_MARKER = '.forge-upgrade-owner';
const MAX_OWNER_BYTES = 1024;
function fdRoot() {
  for (const candidate of ['/proc/self/fd', '/dev/fd']) {
    try {
      if (lstatSync(candidate).isDirectory()) return candidate;
    } catch {}
  }
  return null;
}
function requireSupport() {
  const root = fdRoot();
  if (DIRECTORY === undefined || NOFOLLOW === undefined || root === null) {
    throw new Error(
      'forge-upgrade --apply requires O_DIRECTORY, O_NOFOLLOW, and descriptor-relative paths',
    );
  }
  return root;
}
function sameIdentity(left, right) {
  return left.dev === right.dev && left.ino === right.ino;
}
function directoryStat(path, label) {
  const stat = lstatSync(path, { bigint: true });
  if (stat.isSymbolicLink() || !stat.isDirectory()) {
    throw new Error(`refusing changed directory boundary for ${label}: ${path}`);
  }
  return stat;
}
function descriptorPath(root, fd, leaf = null) {
  const base = join(root, String(fd));
  return leaf === null ? base : join(base, leaf);
}
function openDirectory(path, lexical, label, descriptorRoot, parent = null) {
  let fd;
  try {
    fd = openSync(path, constants.O_RDONLY | DIRECTORY | NOFOLLOW);
    const opened = fstatSync(fd, { bigint: true });
    const anchored = directoryStat(path, label);
    const current = directoryStat(lexical, label);
    if (!opened.isDirectory() || !sameIdentity(opened, anchored)
        || !sameIdentity(opened, current)) {
      throw new Error(`refusing changed directory boundary for ${label}: ${lexical}`);
    }
    return {
      descriptorRoot, dev: opened.dev, fd, ino: opened.ino, name: parent?.name ?? null,
      parentFd: parent?.fd ?? null, path: lexical, relative: parent?.relative ?? '',
    };
  } catch (error) {
    if (fd !== undefined) closeSync(fd);
    throw error;
  }
}
function componentSpecs(targetDir, parent) {
  const root = resolve(targetDir);
  const destinationParent = resolve(parent);
  const suffix = relative(root, destinationParent);
  if (suffix === '..' || suffix.startsWith(`..${sep}`) || isAbsolute(suffix)) {
    throw new Error(`destination parent escapes target directory: ${parent}`);
  }
  const specs = [{ path: root, relative: '', name: null }];
  let cursor = root;
  let relativePath = '';
  for (const name of suffix.split(sep).filter(Boolean)) {
    cursor = join(cursor, name);
    relativePath = relativePath ? join(relativePath, name) : name;
    specs.push({ name, path: cursor, relative: relativePath });
  }
  return specs;
}
function canonicalRelative(value) { return value.split(sep).join('/'); }
function ownerDocument(owner, relativePath, stat) {
  if (!/^[a-f0-9]{32}$/.test(owner ?? '')) {
    throw new Error('invalid upgrade transaction owner');
  }
  return {
    api_version: 'forgeos.scaffold-upgrade-directory-owner/v1',
    dev: String(stat.dev), ino: String(stat.ino), owner,
    relative: canonicalRelative(relativePath),
  };
}
function ownerBytes(owner, relativePath, stat) {
  return Buffer.from(`${JSON.stringify(ownerDocument(owner, relativePath, stat))}\n`);
}
function syncDirectory(fd) {
  fsyncSync(fd);
}
function createOwnedComponent(parent, spec, owner, label) {
  return createOwnedDirectoryComponent(
    parent, spec, label, (stat) => ownerBytes(owner, spec.relative, stat),
  );
}
function decodeOwner(bytes, label) {
  if (bytes.length > MAX_OWNER_BYTES) throw new Error(`oversized ${label}`);
  let document;
  try { document = JSON.parse(bytes); } catch { throw new Error(`malformed ${label}`); }
  const keys = Object.keys(document ?? {}).sort();
  const expected = ['api_version', 'dev', 'ino', 'owner', 'relative'];
  if (JSON.stringify(keys) !== JSON.stringify(expected)
      || document.api_version !== 'forgeos.scaffold-upgrade-directory-owner/v1'
      || !/^[a-f0-9]{32}$/.test(document.owner ?? '')
      || !/^(0|[1-9]\d*)$/.test(document.dev ?? '')
      || !/^[1-9]\d*$/.test(document.ino ?? '')
      || typeof document.relative !== 'string') throw new Error(`malformed ${label}`);
  return document;
}
function readOwnership(path, label, optional = false) {
  let fd;
  try {
    fd = openSync(path, constants.O_RDONLY | NOFOLLOW | NONBLOCK);
    const opened = fstatSync(fd, { bigint: true });
    const lexical = lstatSync(path, { bigint: true });
    if (!opened.isFile() || Number(opened.nlink) < 1 || Number(opened.nlink) > 2
        || Number(opened.mode & 0o777n) !== 0o600 || !sameIdentity(opened, lexical)) {
      throw new Error(`unsafe ${label}`);
    }
    return {
      document: decodeOwner(readFileSync(fd), label),
      identity: Object.freeze({ dev: opened.dev, ino: opened.ino }),
      links: Number(opened.nlink),
    };
  } catch (error) {
    if (optional && error?.code === 'ENOENT') return null;
    throw error;
  } finally {
    if (fd !== undefined) closeSync(fd);
  }
}
function ownerMatches(document, owner, relativePath, stat) {
  return document.owner === owner && document.relative === canonicalRelative(relativePath)
    && document.dev === String(stat.dev) && document.ino === String(stat.ino);
}
function markerMatches(item, owner) {
  const marker = descriptorPath(item.descriptorRoot, item.fd, OWNER_MARKER);
  const record = readOwnership(marker, 'upgrade directory owner', true);
  return record !== null && record.links === 1
    && ownerMatches(record.document, owner, item.relative, item);
}
function openComponent(parent, spec, label, owner, create) {
  const anchored = descriptorPath(parent.descriptorRoot, parent.fd, spec.name);
  try {
    return openDirectory(anchored, spec.path, label, parent.descriptorRoot, {
      fd: parent.fd, name: spec.name, relative: spec.relative,
    });
  } catch (error) {
    if (error?.code !== 'ENOENT' || !create) throw error;
  }
  const created = createOwnedComponent(parent, spec, owner, label);
  const item = openDirectory(anchored, spec.path, label, parent.descriptorRoot, {
    fd: parent.fd, name: spec.name, relative: spec.relative,
  });
  if (created && !markerMatches(item, owner)) {
    closeSync(item.fd);
    throw new Error(`lost ownership of created directory for ${label}: ${spec.path}`);
  }
  return item;
}

export function captureParentBoundary(
  targetDir, destination, label, { create = false, expectedRoot = undefined, owner = null } = {},
) {
  const descriptorRoot = requireSupport();
  const components = [];
  try {
    const specs = componentSpecs(targetDir, dirname(destination));
    components.push(openDirectory(
      specs[0].path, specs[0].path, label, descriptorRoot,
    ));
    if (expectedRoot !== undefined && !sameIdentity(components[0], expectedRoot)) {
      throw new Error(`refusing changed target root for ${label}: ${targetDir}`);
    }
    for (const spec of specs.slice(1)) {
      components.push(openComponent(
        components.at(-1), spec, label, owner, create,
      ));
    }
    const boundary = { components, descriptorRoot, label, owner };
    assertParentBoundary(boundary);
    return boundary;
  } catch (error) {
    let cleanupError = null;
    if (create && owner !== null && components.length > 0) {
      try {
        cleanupOwnedBoundaryDirectories(
          [{ components, descriptorRoot, label, owner }], owner, true,
        );
      } catch (cleanup) { cleanupError = cleanup; }
    }
    for (const item of components.reverse()) {
      try { closeSync(item.fd); } catch {}
    }
    if (cleanupError !== null) {
      throw new Error(`${error.message}; ${cleanupError.message}`);
    }
    throw error;
  }
}
export function assertParentBoundary(boundary) {
  for (const item of boundary.components) {
    const opened = fstatSync(item.fd, { bigint: true });
    const lexical = directoryStat(item.path, boundary.label);
    if (!opened.isDirectory() || opened.dev !== item.dev || opened.ino !== item.ino
        || !sameIdentity(opened, lexical)) {
      throw new Error(
        `refusing changed directory boundary for ${boundary.label}: ${item.path}`,
      );
    }
  }
}
export function anchoredPath(boundary, name) {
  if (!name || name === '.' || name === '..' || name.includes('/') || name.includes('\\')) {
    throw new Error(`unsafe descriptor-relative leaf for ${boundary.label}: ${name}`);
  }
  const parent = boundary.components.at(-1);
  return descriptorPath(boundary.descriptorRoot, parent.fd, name);
}
function ownedComponents(boundaries, owner) {
  const selected = new Map();
  for (const boundary of boundaries) {
    for (const item of boundary.components.slice(1)) {
      if (!selected.has(item.path) && markerMatches(item, owner)) selected.set(item.path, item);
    }
  }
  return [...selected.values()].sort((left, right) => right.path.length - left.path.length);
}
function validateOwnership(record, owner, context, directory, label) {
  if (record === null || !ownerMatches(record.document, owner, context.relative, directory)) {
    throw new Error(`unrecognized ${label} for ${context.relative}`);
  }
}
function openPinnedDirectory(path, label) {
  const lexical = directoryStat(path, label);
  const fd = openSync(path, constants.O_RDONLY | DIRECTORY | NOFOLLOW);
  const opened = fstatSync(fd, { bigint: true });
  if (!sameIdentity(opened, lexical)) {
    closeSync(fd);
    throw new Error(`changed ${label}: ${path}`);
  }
  return { fd, stat: opened };
}
function removeFinishedProof(context, proof, owner) {
  const document = proof.document;
  if (document.owner !== owner
      || document.relative !== canonicalRelative(context.relative)) {
    throw new Error(`unrecognized owned-directory proof for ${context.relative}`);
  }
  removeKnownFile(
    context.proof, proof.identity, `owned-directory proof ${context.relative}`,
    false, context.parentFd,
  );
}
function ensureCleanupProof(context, owner, opened, marker, proof) {
  if (proof !== null) {
    validateOwnership(proof, owner, context, opened.stat, 'owned-directory proof');
    return proof;
  }
  validateOwnership(marker, owner, context, opened.stat, 'owned-directory marker');
  const markerPath = descriptorPath(context.descriptorRoot, opened.fd, OWNER_MARKER);
  linkSync(markerPath, context.proof);
  syncDirectory(context.parentFd);
  return readOwnership(context.proof, 'owned-directory proof');
}
function finishOwnedQuarantine(context, owner, hooks, opened, proof) {
  const markerPath = descriptorPath(context.descriptorRoot, opened.fd, OWNER_MARKER);
  const marker = readOwnership(markerPath, 'owned-directory marker', true);
  const durable = ensureCleanupProof(context, owner, opened, marker, proof);
  if (marker !== null) {
    validateOwnership(marker, owner, context, opened.stat, 'owned-directory marker');
    if (!sameIdentity(marker.identity, durable.identity)) {
      throw new Error(`changed owned-directory proof for ${context.relative}`);
    }
    removeKnownFile(
      markerPath, marker.identity, `owned-directory marker ${context.relative}`,
      false, opened.fd,
    );
    hooks.afterOwnedDirectoryMarkerUnlink?.({
      path: context.path, quarantine: context.quarantine, relative: context.relative,
    });
  }
  hooks.beforeOwnedDirectoryFinalDetach?.({
    path: context.path, quarantine: context.child, relative: context.relative,
  });
  removeKnownDirectory(
    opened.path, opened.stat, `owned-directory quarantine ${context.relative}`,
    false, context.parentFd, ({ quarantine }) => {
      hooks.afterOwnedDirectoryQuarantine?.({
        path: context.path, quarantine, relative: context.relative,
      });
    },
  );
  removeKnownFile(
    context.proof, durable.identity, `owned-directory proof ${context.relative}`,
    false, context.parentFd,
  );
}

function ownedDirectoryCandidate(context, proof) {
  const expected = {
    dev: BigInt(proof.document.dev), ino: BigInt(proof.document.ino),
  };
  const paths = [context.child, ...removalCandidates(context.child)];
  const candidates = [];
  for (const path of paths) {
    try { candidates.push({ path, stat: directoryStat(path, 'owned-directory cleanup') }); }
    catch (error) { if (error?.code !== 'ENOENT') throw error; }
  }
  const matching = candidates.filter((item) => sameIdentity(item.stat, expected));
  if (matching.length > 1) throw new Error(`ambiguous owned directory ${context.relative}`);
  if (matching.length === 1) return matching[0];
  if (candidates.length > 0) {
    throw new Error(`unrecognized owned-directory replacement for ${context.relative}`);
  }
  return null;
}

function resumeOwnedQuarantine(context, owner, hooks) {
  const proof = readOwnership(context.proof, 'owned-directory proof', true);
  if (proof === null) return false;
  const candidate = ownedDirectoryCandidate(context, proof);
  if (candidate === null) {
    removeFinishedProof(context, proof, owner);
    return true;
  }
  const opened = openPinnedDirectory(
    candidate.path, `owned-directory quarantine ${context.relative}`,
  );
  opened.path = candidate.path;
  try { finishOwnedQuarantine(context, owner, hooks, opened, proof); } finally {
    closeSync(opened.fd);
  }
  return true;
}
function quarantineOwnedItem(item, owner, hooks) {
  const context = ownedDirectoryCleanupContext(item, owner);
  if (resumeOwnedQuarantine(context, owner, hooks)) return;
  if (!markerMatches(item, owner)) throw new Error(`lost owned directory ${item.relative}`);
  const current = directoryStat(context.child, `owned directory ${item.relative}`);
  if (current.dev !== item.dev || current.ino !== item.ino) {
    throw new Error(`preserved concurrent directory replacement at ${context.child}`);
  }
  const occupants = readdirSync(descriptorPath(item.descriptorRoot, item.fd))
    .filter((name) => name !== OWNER_MARKER);
  if (occupants.length) {
    if (occupants.some((name) => name.startsWith(`.forge-upgrade-txn-${owner.slice(0, 12)}`))) {
      throw new Error(`preserved transaction directory artifacts for ${item.relative}`);
    }
    removeOwnedMarker(item, owner);
    return;
  }
  hooks.beforeOwnedDirectoryQuarantineReservation?.({
    path: item.path, quarantine: context.quarantine, relative: context.relative,
  });
  finishOwnedQuarantine(
    context, owner, hooks, { fd: item.fd, path: context.child, stat: item }, null,
  );
}

function removeOwnedMarker(item, owner) {
  const path = descriptorPath(item.descriptorRoot, item.fd, OWNER_MARKER);
  const marker = readOwnership(path, 'upgrade directory owner', true);
  if (marker === null || marker.links !== 1
      || !ownerMatches(marker.document, owner, item.relative, item)) return;
  removeKnownFile(path, marker.identity, `owned directory marker ${item.relative}`, false, item.fd);
}

export function cleanupOwnedBoundaryDirectories(
  boundaries, owner, removeEmpty, hooks = {},
) {
  const errors = [];
  for (const item of ownedComponents(boundaries, owner)) {
    try {
      if (removeEmpty) quarantineOwnedItem(item, owner, hooks);
      else removeOwnedMarker(item, owner);
    } catch (error) { errors.push(error); break; }
  }
  if (errors.length) throw new Error(
    `owned directory cleanup failed: ${errors.map((error) => error.message).join('; ')}`,
  );
}

function openRecoveryItem(boundary, targetDir, relativePath) {
  const parent = boundary.components.at(-1);
  const name = basename(relativePath);
  const path = join(targetDir, relativePath);
  try {
    return openDirectory(
      descriptorPath(boundary.descriptorRoot, parent.fd, name), path,
      `recovery ${relativePath}`, boundary.descriptorRoot,
      { fd: parent.fd, name, relative: relativePath },
    );
  } catch (error) {
    if (error?.code === 'ENOENT') return null;
    throw error;
  }
}

function recoverOneDirectory(targetDir, relativePath, owner, removeEmpty, hooks, expectedRoot) {
  const parentRelative = dirname(relativePath);
  const destination = join(targetDir, parentRelative === '.' ? '' : parentRelative, '.probe');
  let boundary;
  try {
    boundary = captureParentBoundary(
      targetDir, destination, `recovery ${relativePath}`, { expectedRoot },
    );
  } catch (error) {
    if (['ENOENT', 'ENOTDIR'].includes(error?.code)) return;
    throw error;
  }
  let item = null;
  try {
    const parent = boundary.components.at(-1);
    const context = ownedDirectoryCleanupContext({
      descriptorRoot: boundary.descriptorRoot, name: basename(relativePath),
      parentFd: parent.fd, path: join(targetDir, relativePath), relative: relativePath,
    }, owner);
    if (resumeOwnedQuarantine(context, owner, hooks)) return;
    item = openRecoveryItem(boundary, targetDir, relativePath);
    if (item === null || !markerMatches(item, owner)) return;
    if (removeEmpty) quarantineOwnedItem(item, owner, hooks);
    else removeOwnedMarker(item, owner);
  } finally {
    if (item !== null) closeSync(item.fd);
    closeParentBoundary(boundary);
  }
}

function recoverUnstartedStage(targetDir, entry, authority, intent, hooks, expectedRoot) {
  let boundary;
  try {
    boundary = captureParentBoundary(
      targetDir, join(targetDir, entry.rel), `unstarted recovery ${entry.rel}`,
      { expectedRoot },
    );
  } catch (error) {
    if (['ENOENT', 'ENOTDIR'].includes(error?.code)) return;
    throw error;
  }
  try { cleanupUnstartedUpgradeStage(boundary, entry, authority, intent, hooks); }
  finally { closeParentBoundary(boundary); }
}

export function cleanupUnstartedUpgradeStages(
  targetDir, document, authorities, intents, hooks, expectedRoot,
) {
  for (const entry of [...document.entries].reverse()) {
    const authority = authorities === null ? null : authorities.get(entry.stage_name);
    if (authority === undefined) throw new Error(`missing prepared stage authority for ${entry.rel}`);
    recoverUnstartedStage(
      targetDir, entry, authority, intents.get(entry.stage_name) ?? null, hooks, expectedRoot,
    );
  }
  recoverOwnedDirectories(
    targetDir, document.directories, document.transaction_id, true, hooks, expectedRoot,
  );
}

export function recoverOwnedDirectories(
  targetDir, relativePaths, owner, removeEmpty, hooks = {}, expectedRoot = undefined,
) {
  const ordered = [...new Set(relativePaths)].sort((a, b) => b.length - a.length);
  for (const relativePath of ordered) {
    recoverOneDirectory(targetDir, relativePath, owner, removeEmpty, hooks, expectedRoot);
  }
}

export function closeParentBoundary(boundary) {
  const errors = [];
  for (const item of [...boundary.components].reverse()) {
    try { closeSync(item.fd); } catch (error) { errors.push(error); }
  }
  if (errors.length > 0) {
    throw new Error(`directory boundary cleanup failed: ${errors.map((e) => e.message).join('; ')}`);
  }
}
