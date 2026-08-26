// Portable bounded candidate observation for hosts without descriptor-relative
// directory traversal. Linux uses the stronger descriptor implementation; this
// path keeps formal acceptance live elsewhere without claiming an atomic FS snapshot.
import { createHash } from 'node:crypto';
import {
  closeSync, constants as fsConstants, fstatSync, lstatSync, openSync, readSync,
} from 'node:fs';
import { join } from 'node:path';

function assertParents(root, relativePath) {
  const parts = relativePath.split('/');
  let current = root;
  for (const part of parts.slice(0, -1)) {
    current = join(current, part);
    const info = lstatSync(current, { bigint: true });
    if (!info.isDirectory() || info.isSymbolicLink()) {
      throw new Error(`candidate parent is unsafe: ${relativePath}`);
    }
  }
}

function digestRegular(path, relativePath, before, helpers) {
  const noFollow = Number.isInteger(fsConstants.O_NOFOLLOW) ? fsConstants.O_NOFOLLOW : 0;
  const descriptor = openSync(path, fsConstants.O_RDONLY | noFollow);
  try {
    const opened = fstatSync(descriptor, { bigint: true });
    if (!helpers.sameStat(before, opened)) throw new Error(`candidate raced open: ${relativePath}`);
    if (opened.nlink !== 1n) {
      throw new Error(`candidate hardlinks are unsupported: ${relativePath}`);
    }
    if (opened.size > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new Error(`candidate file is too large to fingerprint: ${relativePath}`);
    }
    const digest = createHash('sha256');
    const buffer = Buffer.allocUnsafe(64 * 1024);
    let offset = 0;
    while (offset < Number(opened.size)) {
      const count = readSync(
        descriptor, buffer, 0, Math.min(buffer.length, Number(opened.size) - offset), null,
      );
      if (count <= 0) throw new Error(`candidate file ended early: ${relativePath}`);
      digest.update(buffer.subarray(0, count));
      offset += count;
    }
    const after = fstatSync(descriptor, { bigint: true });
    if (!helpers.sameStat(opened, after)) {
      throw new Error(`candidate changed while read: ${relativePath}`);
    }
    return digest.digest('hex');
  } finally { closeSync(descriptor); }
}

function observeEntry(root, candidate, helpers) {
  const safe = helpers.safePath(root, candidate);
  assertParents(root, safe.relative);
  const before = lstatSync(safe.path, { bigint: true });
  let row;
  if (before.isFile()) {
    const digest = digestRegular(safe.path, safe.relative, before, helpers);
    row = `file\0${safe.relative}\0${helpers.statIdentity(
      before, helpers.FINGERPRINT_STAT_FIELDS,
    )}\0${digest}`;
  } else if (before.isSymbolicLink()) {
    throw new Error(`candidate symlinks are unsupported: ${safe.relative}`);
  } else if (before.isDirectory()) {
    row = `directory\0${safe.relative}\0${helpers.statIdentity(
      before, helpers.DIRECTORY_IDENTITY_FIELDS,
    )}`;
  } else {
    throw new Error(`candidate entry is unsupported: ${safe.relative}`);
  }
  const after = lstatSync(safe.path, { bigint: true });
  if (!helpers.sameStat(before, after)) {
    throw new Error(`candidate changed while observed: ${safe.relative}`);
  }
  return { row, validation: helpers.statIdentity(after, helpers.STAT_FIELDS) };
}

function observeRows(root, candidates, helpers) {
  const rows = new Map();
  const validations = new Map();
  for (const candidate of candidates.paths) {
    const observed = observeEntry(root, candidate, helpers);
    rows.set(candidate, observed.row);
    validations.set(candidate, observed.validation);
  }
  return { rows, validations };
}

function sameListing(left, right) {
  return left.length === right.length
    && left.every((value, index) => value === right[index]);
}

function directoryPath(root, relativePath, helpers) {
  if (relativePath === '') return root;
  assertParents(root, relativePath);
  return helpers.safePath(root, relativePath).path;
}

function verifyDirectory(root, relativePath, expected, helpers) {
  const path = directoryPath(root, relativePath, helpers);
  const before = lstatSync(path, { bigint: true });
  if (!before.isDirectory() || before.isSymbolicLink()) {
    throw new Error(`candidate directory is unsafe: ${relativePath || '.'}`);
  }
  const actual = helpers.listingIdentity(helpers.visibleEntries(path));
  const after = lstatSync(path, { bigint: true });
  if (!helpers.sameStat(before, after) || !sameListing(actual, expected)) {
    throw new Error(`candidate directory changed during fingerprint: ${relativePath || '.'}`);
  }
}

function verifySnapshot(root, env, before, helpers) {
  const after = helpers.candidatePaths(root, env);
  if (!sameListing(before.paths, after.paths)
      || before.directories.size !== after.directories.size) {
    throw new Error('candidate directory changed during fingerprint');
  }
  const directories = [...before.directories].sort(([left], [right]) => {
    const depth = (path) => path === '' ? -1 : path.split('/').length;
    return depth(right) - depth(left) || left.localeCompare(right);
  });
  for (const [path, listing] of directories) {
    const current = after.directories.get(path);
    if (!current || !sameListing(listing, current)) {
      throw new Error(`candidate directory changed during fingerprint: ${path || '.'}`);
    }
    verifyDirectory(root, path, current, helpers);
  }
}

function compareRows(expected, current) {
  for (const [path, row] of expected) {
    if (current.get(path) !== row) {
      throw new Error(`candidate entry changed during fingerprint: ${path}`);
    }
  }
}

function verifyMetadata(root, validations, helpers) {
  const ordered = [...validations].sort(([left], [right]) => {
    const depth = (path) => path.split('/').length;
    return depth(right) - depth(left) || right.localeCompare(left);
  });
  for (const [relativePath, expected] of ordered) {
    const safe = helpers.safePath(root, relativePath);
    assertParents(root, safe.relative);
    const before = lstatSync(safe.path, { bigint: true });
    if (helpers.statIdentity(before, helpers.STAT_FIELDS) !== expected
        || before.isSymbolicLink() || (!before.isFile() && !before.isDirectory())) {
      throw new Error(`candidate entry changed after revalidation: ${relativePath}`);
    }
    if (before.isFile()) {
      const noFollow = Number.isInteger(fsConstants.O_NOFOLLOW) ? fsConstants.O_NOFOLLOW : 0;
      const descriptor = openSync(safe.path, fsConstants.O_RDONLY | noFollow);
      try {
        if (!helpers.sameStat(before, fstatSync(descriptor, { bigint: true }))) {
          throw new Error(`candidate raced final open: ${relativePath}`);
        }
      } finally { closeSync(descriptor); }
    }
    const after = lstatSync(safe.path, { bigint: true });
    if (!helpers.sameStat(before, after)) {
      throw new Error(`candidate changed during final validation: ${relativePath}`);
    }
  }
}

function stableObservation(root, env, helpers) {
  const rootBefore = lstatSync(root, { bigint: true });
  if (!rootBefore.isDirectory() || rootBefore.isSymbolicLink()) {
    throw new Error(`candidate root is unsafe: ${root}`);
  }
  const candidates = helpers.candidatePaths(root, env);
  const first = observeRows(root, candidates, helpers);
  verifySnapshot(root, env, candidates, helpers);
  const second = observeRows(root, candidates, helpers);
  compareRows(first.rows, second.rows);
  verifySnapshot(root, env, candidates, helpers);
  verifyMetadata(root, second.validations, helpers);
  const rootAfter = lstatSync(root, { bigint: true });
  if (!helpers.sameStat(rootBefore, rootAfter)) {
    throw new Error('candidate root changed during fingerprint');
  }
  return { rootBefore, rows: first.rows };
}

export function portableCandidateFingerprint(root, env, helpers) {
  const observed = stableObservation(root, env, helpers);
  const hash = createHash('sha256').update('forge-source-candidate/v1\0');
  hash.update(`root\0${helpers.statIdentity(
    observed.rootBefore, helpers.DIRECTORY_IDENTITY_FIELDS,
  )}\0`);
  for (const row of observed.rows.values()) hash.update(row).update('\0');
  return hash.digest('hex');
}

export function portableCandidateInventory(root, env, helpers) {
  return stableObservation(root, env, helpers).rows;
}
