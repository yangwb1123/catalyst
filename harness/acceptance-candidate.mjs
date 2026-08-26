// Stable source-candidate fingerprint shared by formal drift detection and the
// advisory cache. It unions Git tracked + untracked/non-ignored paths with the
// visible scanner tree (including ignored inputs), excluding only directories
// every cacheable static scanner skips as generated/vendor/runtime stores.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import {
  closeSync, constants as fsConstants, existsSync, fstatSync, lstatSync, openSync,
  readdirSync, readSync,
} from 'node:fs';
import { isAbsolute, join, normalize, relative, resolve, sep } from 'node:path';

import {
  portableCandidateFingerprint, portableCandidateInventory,
} from './acceptance-candidate-portable.mjs';

// Directories never part of a static-source candidate: VCS metadata, build
// products, dependency trees and runtime stores. Test fixtures remain in the
// candidate: Go tooling may touch their ctime during builds (overlay envs
// included), so ctime is deliberately omitted from stable rows below while
// bytes, path, inode, mode, size and mtime remain bound.
const GENERATED_DIRS = new Set([
  '.git', '.forge', 'node_modules', 'target', 'coverage', 'dist', 'build',
  '.next', '__pycache__', '.coverage', 'coverage.json', 'coverage.out',
]);
const GENERATED_PREFIXES = ['.forge-coverage-'];
const STAT_FIELDS = [
  'dev', 'ino', 'mode', 'nlink', 'uid', 'gid', 'size', 'mtimeNs', 'ctimeNs',
];
const FINGERPRINT_STAT_FIELDS = STAT_FIELDS.filter((field) => field !== 'ctimeNs');
const DIRECTORY_IDENTITY_FIELDS = ['dev', 'ino', 'mode', 'uid', 'gid'];
const GIT_LOCATION_ENV = new Set([
  'GIT_DIR', 'GIT_WORK_TREE', 'GIT_COMMON_DIR', 'GIT_INDEX_FILE',
  'GIT_OBJECT_DIRECTORY', 'GIT_ALTERNATE_OBJECT_DIRECTORIES',
  'GIT_CEILING_DIRECTORIES', 'GIT_DISCOVERY_ACROSS_FILESYSTEM', 'GIT_PREFIX',
]);

function sameStat(left, right) {
  return STAT_FIELDS.every((field) => left[field] === right[field]);
}

function statIdentity(info, fields = STAT_FIELDS) {
  return fields.map((field) => `${field}=${info[field]}`).join('\0');
}

function safePath(root, candidate) {
  if (!candidate || candidate.includes('\0') || isAbsolute(candidate)) {
    throw new Error(`candidate path is unsafe: ${JSON.stringify(candidate)}`);
  }
  const path = resolve(root, normalize(candidate));
  const rel = relative(root, path);
  if (!rel || rel === '..' || rel.startsWith(`..${sep}`) || isAbsolute(rel)) {
    throw new Error(`candidate path escaped root: ${JSON.stringify(candidate)}`);
  }
  return { path, relative: rel.split(sep).join('/') };
}

function repositoryGitEnvironment(env) {
  const clean = { ...env, GIT_OPTIONAL_LOCKS: '0' };
  for (const key of GIT_LOCATION_ENV) delete clean[key];
  return clean;
}

function runGit(root, env, args, maxBuffer = 1024 * 1024) {
  return spawnSync('git', ['-c', `core.worktree=${resolve(root)}`, ...args], {
    cwd: root, env: repositoryGitEnvironment(env), encoding: 'buffer',
    timeout: 60_000, maxBuffer,
  });
}

function gitPaths(root, env) {
  const marker = join(root, '.git');
  let markerInfo;
  try { markerInfo = lstatSync(marker); }
  catch (error) {
    if (error?.code === 'ENOENT') return null;
    throw error;
  }
  if (markerInfo.isSymbolicLink() || (!markerInfo.isFile() && !markerInfo.isDirectory())) {
    throw new Error('Git candidate marker is unsafe');
  }
  const top = runGit(root, env, ['rev-parse', '--show-toplevel']);
  if (top.error || top.status !== 0 || resolve(top.stdout.toString('utf8').trim()) !== resolve(root)) {
    throw new Error(`Git candidate root verification failed: ${top.error?.message ?? top.stderr}`);
  }
  const run = runGit(root, env, ['ls-files', '-co', '--exclude-standard', '-z'], 64 * 1024 * 1024);
  if (run.error || run.status !== 0) {
    throw new Error(`Git candidate enumeration failed: ${run.error?.message ?? run.stderr}`);
  }
  const deletedRun = runGit(root, env, ['ls-files', '--deleted', '-z'], 64 * 1024 * 1024);
  if (deletedRun.error || deletedRun.status !== 0) {
    throw new Error(`Git deletion enumeration failed: ${deletedRun.error?.message ?? deletedRun.stderr}`);
  }
  const deleted = new Set(deletedRun.stdout.toString('utf8').split('\0').filter(Boolean));
  return [...new Set(run.stdout.toString('utf8').split('\0').filter(Boolean))]
    .filter((path) => !deleted.has(path)).sort();
}

function entryKind(entry) {
  if (entry.isFile()) return 'file';
  if (entry.isDirectory()) return 'directory';
  if (entry.isSymbolicLink()) return 'symlink';
  return 'other';
}

function generatedName(name) {
  return GENERATED_DIRS.has(name)
    || GENERATED_PREFIXES.some((prefix) => name.startsWith(prefix));
}

function visibleEntries(current) {
  return readdirSync(current, { withFileTypes: true })
    .filter((entry) => !generatedName(entry.name))
    .sort((left, right) => left.name.localeCompare(right.name));
}

function listingIdentity(entries) {
  return entries.map((entry) => `${entryKind(entry)}\0${entry.name}`);
}

function walkPaths(current, root, paths, directories) {
  const entries = visibleEntries(current);
  directories.set(relative(root, current).split(sep).join('/'), listingIdentity(entries));
  for (const entry of entries) {
    const path = join(current, entry.name);
    const rel = relative(root, path).split(sep).join('/');
    paths.push(rel);
    if (entry.isDirectory()) walkPaths(path, root, paths, directories);
  }
}

function generatedPath(path) {
  return path.split('/').some(generatedName);
}

export function candidateEventRelevant(path) {
  const normalized = String(path).replaceAll('\\', '/');
  return normalized !== '' && !generatedPath(normalized);
}

function candidatePaths(root, env) {
  const listed = gitPaths(root, env)?.filter((path) => !generatedPath(path));
  const paths = [];
  const directories = new Map();
  walkPaths(root, root, paths, directories);
  return {
    directories,
    paths: [...new Set([...(listed ?? []), ...paths])].sort(),
  };
}

const PORTABLE_HELPERS = {
  candidatePaths, safePath, visibleEntries, listingIdentity, sameStat, statIdentity,
  STAT_FIELDS, FINGERPRINT_STAT_FIELDS, DIRECTORY_IDENTITY_FIELDS,
};

function descriptorTraversalAvailable() {
  return Number.isInteger(fsConstants.O_DIRECTORY)
    && Number.isInteger(fsConstants.O_NOFOLLOW)
    && ['/proc/self/fd', '/dev/fd'].some((path) => existsSync(path));
}

function descriptorCapabilityError(message) {
  const error = new Error(message);
  error.code = 'FORGE_DESCRIPTOR_TRAVERSAL_UNAVAILABLE';
  return error;
}

function descriptorAnchor(descriptor) {
  for (const base of ['/proc/self/fd', '/dev/fd']) {
    const path = join(base, String(descriptor));
    try {
      if (existsSync(path)) {
        lstatSync(join(path, '.'), { bigint: true });
        return path;
      }
    } catch { /* try the next descriptor filesystem */ }
  }
  throw descriptorCapabilityError('candidate fingerprint cannot address directory descriptors');
}

function openDirectory(path, relativePath, expected = null) {
  if (!Number.isInteger(fsConstants.O_DIRECTORY) || !Number.isInteger(fsConstants.O_NOFOLLOW)) {
    throw descriptorCapabilityError('candidate fingerprint lacks no-follow directory support');
  }
  const descriptor = openSync(
    path, fsConstants.O_RDONLY | fsConstants.O_DIRECTORY | fsConstants.O_NOFOLLOW,
  );
  try {
    const info = fstatSync(descriptor, { bigint: true });
    if (!info.isDirectory() || (expected && !sameStat(expected, info))) {
      throw new Error(`candidate directory raced open: ${relativePath}`);
    }
    return { descriptor, anchor: descriptorAnchor(descriptor) };
  } catch (error) {
    closeSync(descriptor);
    throw error;
  }
}

function anchoredLeaf(rootAnchor, relativePath) {
  const parts = relativePath.split('/');
  const opened = [];
  let anchor = rootAnchor;
  try {
    for (const part of parts.slice(0, -1)) {
      const directory = openDirectory(join(anchor, part), relativePath);
      opened.push(directory.descriptor);
      anchor = directory.anchor;
    }
    return {
      path: join(anchor, parts.at(-1)),
      close: () => opened.reverse().forEach((descriptor) => closeSync(descriptor)),
    };
  } catch (error) {
    opened.reverse().forEach((descriptor) => closeSync(descriptor));
    throw error;
  }
}

function digestRegular(path, relativePath, before) {
  if (!Number.isInteger(fsConstants.O_NOFOLLOW)) {
    throw new Error('candidate fingerprint requires no-follow file support');
  }
  const descriptor = openSync(path, fsConstants.O_RDONLY | fsConstants.O_NOFOLLOW);
  try {
    const opened = fstatSync(descriptor, { bigint: true });
    if (!sameStat(before, opened)) throw new Error(`candidate raced open: ${relativePath}`);
    if (opened.nlink !== 1n) {
      throw new Error(`candidate hardlinks are unsupported: ${relativePath}`);
    }
    if (opened.size > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new Error(`candidate file is too large to fingerprint: ${relativePath}`);
    }
    const digest = createHash('sha256');
    const buffer = Buffer.allocUnsafe(64 * 1024);
    let offset = 0;
    const size = Number(opened.size);
    while (offset < size) {
      const count = readSync(descriptor, buffer, 0, Math.min(buffer.length, size - offset), null);
      if (count <= 0) throw new Error(`candidate file ended early: ${relativePath}`);
      digest.update(buffer.subarray(0, count));
      offset += count;
    }
    const after = fstatSync(descriptor, { bigint: true });
    if (!sameStat(opened, after)) throw new Error(`candidate changed while read: ${relativePath}`);
    return digest.digest('hex');
  } finally { closeSync(descriptor); }
}

function digestEntry(root, rootAnchor, candidate, validations = null) {
  const safe = safePath(root, candidate);
  const anchored = anchoredLeaf(rootAnchor, safe.relative);
  try {
    const before = lstatSync(anchored.path, { bigint: true });
    if (before.isFile()) {
      const digest = digestRegular(anchored.path, safe.relative, before);
      validations?.set(safe.relative, statIdentity(before));
      return `file\0${safe.relative}\0${statIdentity(before, FINGERPRINT_STAT_FIELDS)}\0${digest}`;
    }
    if (before.isSymbolicLink()) {
      throw new Error(`candidate symlinks are unsupported: ${safe.relative}`);
    }
    if (before.isDirectory()) {
      const opened = openDirectory(anchored.path, safe.relative, before);
      closeSync(opened.descriptor);
      validations?.set(safe.relative, statIdentity(before));
      return `directory\0${safe.relative}\0${statIdentity(before, DIRECTORY_IDENTITY_FIELDS)}`;
    }
    throw new Error(`candidate entry is unsupported: ${safe.relative}`);
  } finally { anchored.close(); }
}

function digestCandidateRows(root, rootAnchor, candidates) {
  const rows = new Map();
  for (const candidate of candidates.paths) {
    rows.set(candidate, digestEntry(root, rootAnchor, candidate));
  }
  return rows;
}

function verifyCandidateRows(root, rootAnchor, candidates, expected) {
  const validations = new Map();
  for (const candidate of candidates.paths) {
    if (digestEntry(root, rootAnchor, candidate, validations) !== expected.get(candidate)) {
      throw new Error(`candidate entry changed during fingerprint: ${candidate}`);
    }
  }
  return validations;
}

function verifyCandidateMetadata(root, rootAnchor, validations) {
  const ordered = [...validations].sort(([left], [right]) => {
    const depth = (path) => path.split('/').length;
    return depth(right) - depth(left) || right.localeCompare(left);
  });
  for (const [relativePath, expected] of ordered) {
    const anchored = anchoredLeaf(rootAnchor, relativePath);
    try {
      const before = lstatSync(anchored.path, { bigint: true });
      if (statIdentity(before) !== expected || before.isSymbolicLink()) {
        throw new Error(`candidate entry changed after revalidation: ${relativePath}`);
      }
      if (before.isFile()) {
        const descriptor = openSync(anchored.path, fsConstants.O_RDONLY | fsConstants.O_NOFOLLOW);
        try {
          if (!sameStat(before, fstatSync(descriptor, { bigint: true }))) {
            throw new Error(`candidate raced final open: ${relativePath}`);
          }
        } finally { closeSync(descriptor); }
      } else if (before.isDirectory()) {
        const opened = openDirectory(anchored.path, relativePath, before);
        closeSync(opened.descriptor);
      } else {
        throw new Error(`candidate entry is unsupported: ${relativePath}`);
      }
      if (!sameStat(before, lstatSync(anchored.path, { bigint: true }))) {
        throw new Error(`candidate changed during final validation: ${relativePath}`);
      }
    } finally { anchored.close(); }
  }
}

function verifyDirectoryListings(rootAnchor, directories) {
  const deepestFirst = [...directories].sort(([left], [right]) => {
    const depth = (path) => path === '' ? -1 : path.split('/').length;
    return depth(right) - depth(left) || left.localeCompare(right);
  });
  for (const [relativePath, expected] of deepestFirst) {
    if (relativePath === '') {
      const actual = listingIdentity(visibleEntries(rootAnchor));
      if (JSON.stringify(actual) !== JSON.stringify(expected)) {
        throw new Error('candidate directory changed during fingerprint: .');
      }
      continue;
    }
    const anchored = anchoredLeaf(rootAnchor, relativePath);
    try {
      const before = lstatSync(anchored.path, { bigint: true });
      const opened = openDirectory(anchored.path, relativePath, before);
      try {
        const actual = listingIdentity(visibleEntries(opened.anchor));
        if (JSON.stringify(actual) !== JSON.stringify(expected)) {
          throw new Error(`candidate directory changed during fingerprint: ${relativePath}`);
        }
      } finally { closeSync(opened.descriptor); }
    } finally { anchored.close(); }
  }
}

function sameListing(left, right) {
  return left.length === right.length
    && left.every((value, index) => value === right[index]);
}

function verifyCandidateSnapshot(root, env, rootAnchor, before) {
  const after = candidatePaths(root, env);
  if (!sameListing(before.paths, after.paths)
      || before.directories.size !== after.directories.size) {
    throw new Error('candidate directory changed during fingerprint');
  }
  for (const [path, listing] of before.directories) {
    const current = after.directories.get(path);
    if (!current || !sameListing(listing, current)) {
      throw new Error(`candidate directory changed during fingerprint: ${path || '.'}`);
    }
  }
  verifyDirectoryListings(rootAnchor, after.directories);
}

function verifyCandidateRoot(root, rootDirectory, before) {
  const openedAfter = fstatSync(rootDirectory.descriptor, { bigint: true });
  const pathAfter = lstatSync(root, { bigint: true });
  if (!sameStat(before, openedAfter) || !sameStat(before, pathAfter)) {
    throw new Error('candidate root changed during fingerprint');
  }
}

function descriptorCandidateFingerprint(root, env) {
  const rootBefore = lstatSync(root, { bigint: true });
  if (!rootBefore.isDirectory() || rootBefore.isSymbolicLink()) {
    throw new Error(`candidate root is unsafe: ${root}`);
  }
  const rootDirectory = openDirectory(root, '.', rootBefore);
  try {
    const candidates = candidatePaths(root, env);
    const rows = digestCandidateRows(root, rootDirectory.anchor, candidates);
    verifyCandidateSnapshot(root, env, rootDirectory.anchor, candidates);
    const validations = verifyCandidateRows(root, rootDirectory.anchor, candidates, rows);
    verifyCandidateSnapshot(root, env, rootDirectory.anchor, candidates);
    verifyCandidateMetadata(root, rootDirectory.anchor, validations);
    verifyCandidateRoot(root, rootDirectory, rootBefore);
    const hash = createHash('sha256').update('forge-source-candidate/v1\0');
    hash.update(`root\0${statIdentity(rootBefore, DIRECTORY_IDENTITY_FIELDS)}\0`);
    for (const row of rows.values()) hash.update(row).update('\0');
    return hash.digest('hex');
  } finally {
    closeSync(rootDirectory.descriptor);
  }
}

export function candidateFingerprintPortable(root, env = process.env) {
  return portableCandidateFingerprint(root, env, PORTABLE_HELPERS);
}

export function candidateFingerprint(root, env = process.env) {
  if (!descriptorTraversalAvailable()) return candidateFingerprintPortable(root, env);
  try { return descriptorCandidateFingerprint(root, env); }
  catch (error) {
    if (error?.code === 'FORGE_DESCRIPTOR_TRAVERSAL_UNAVAILABLE') {
      return candidateFingerprintPortable(root, env);
    }
    throw error;
  }
}

// Diagnostic inventory: relpath -> fingerprint row for every candidate path. Used
// only to explain a formal-acceptance before/after candidate mismatch (what
// changed, not just that something changed). Fingerprint bytes are unchanged:
// candidateFingerprint hashes exactly these rows in the same order.
function descriptorCandidateInventory(root, env) {
  const rootBefore = lstatSync(root, { bigint: true });
  if (!rootBefore.isDirectory() || rootBefore.isSymbolicLink()) {
    throw new Error(`candidate root is unsafe: ${root}`);
  }
  const rootDirectory = openDirectory(root, '.', rootBefore);
  try {
    const candidates = candidatePaths(root, env);
    const rows = digestCandidateRows(root, rootDirectory.anchor, candidates);
    verifyCandidateSnapshot(root, env, rootDirectory.anchor, candidates);
    const validations = verifyCandidateRows(root, rootDirectory.anchor, candidates, rows);
    verifyCandidateSnapshot(root, env, rootDirectory.anchor, candidates);
    verifyCandidateMetadata(root, rootDirectory.anchor, validations);
    verifyCandidateRoot(root, rootDirectory, rootBefore);
    return rows;
  } finally {
    closeSync(rootDirectory.descriptor);
  }
}

export function candidateInventory(root, env = process.env) {
  if (!descriptorTraversalAvailable()) {
    return portableCandidateInventory(root, env, PORTABLE_HELPERS);
  }
  try { return descriptorCandidateInventory(root, env); }
  catch (error) {
    if (error?.code === 'FORGE_DESCRIPTOR_TRAVERSAL_UNAVAILABLE') {
      return portableCandidateInventory(root, env, PORTABLE_HELPERS);
    }
    throw error;
  }
}

export function candidateInventoryDiff(before, after) {
  const rows = [];
  for (const key of new Set([...before.keys(), ...after.keys()])) {
    if (!after.has(key)) rows.push(`- ${key}`);
    else if (!before.has(key)) rows.push(`+ ${key}`);
    else if (before.get(key) !== after.get(key)) rows.push(`* ${key}`);
  }
  return rows;
}
