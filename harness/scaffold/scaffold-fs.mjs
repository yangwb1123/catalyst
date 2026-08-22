// Shared no-follow copy/enumeration primitives for scaffold and upgrade.
// Source traversal and destination publication intentionally use one policy.
import {
  chmodSync,
  closeSync,
  constants,
  fchmodSync,
  fstatSync,
  ftruncateSync,
  linkSync,
  lstatSync,
  mkdtempSync,
  mkdirSync,
  openSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmdirSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import {
  dirname, join, parse, resolve,
} from 'node:path';

// Reject a lexical destination whose own path OR any existing ancestor is a
// symlink. resolve() is intentionally lexical (it must not follow the link we
// are trying to detect); walking from the filesystem root catches targetDir
// itself, a symlinked parent above targetDir, and intermediate paths such as
// targetDir/harness before any copy/prune can traverse them.
export function assertNoSymlinkComponents(path, label = path) {
  const absolute = resolve(path);
  const descriptor = absolute.match(
    /^(\/(?:proc\/self\/fd|dev\/fd)\/((?:0|[1-9]\d*)))(?:\/(.*))?$/,
  );
  const parsed = parse(absolute);
  let cursor = descriptor?.[1] ?? parsed.root;
  const parts = descriptor === null
    ? absolute.slice(parsed.root.length).split(/[\\/]/).filter(Boolean)
    : (descriptor[3] ?? '').split('/').filter(Boolean);
  if (descriptor !== null) {
    const fd = Number(descriptor[2]);
    if (!Number.isSafeInteger(fd) || !fstatSync(fd).isDirectory()) {
      throw new Error(`refusing unsafe descriptor root for ${label}: ${cursor}`);
    }
  }
  for (let i = 0; i < parts.length; i++) {
    cursor = join(cursor, parts[i]);
    let st;
    try {
      st = lstatSync(cursor);
    } catch (err) {
      if (err?.code === 'ENOENT') break;
      throw new Error(`cannot safely inspect ${label}: ${err.message}`);
    }
    if (st.isSymbolicLink()) {
      throw new Error(`refusing unsafe symlink path for ${label}: ${cursor}`);
    }
    if (i < parts.length - 1 && !st.isDirectory()) {
      throw new Error(`refusing unsafe non-directory ancestor for ${label}: ${cursor}`);
    }
  }
}

function validateRegularStat(st, path, label) {
  if (st.isSymbolicLink()) {
    throw new Error(`refusing unsafe symlink path for ${label}: ${path}`);
  }
  if (!st.isFile()) {
    throw new Error(`refusing unsafe non-file path for ${label}: ${path}`);
  }
  if (Number(st.nlink) > 1) {
    throw new Error(`refusing unsafe hardlink path for ${label}: ${path}`);
  }
  return st;
}

function inspectRegularFile(path, label, allowMissing = false) {
  let st;
  try {
    st = lstatSync(path);
  } catch (err) {
    if (allowMissing && err?.code === 'ENOENT') return null;
    throw new Error(`cannot safely inspect ${label}: ${err.message}`);
  }
  return validateRegularStat(st, path, label);
}

function regularFile(path, label) {
  return inspectRegularFile(path, label, false);
}

// Validate the complete copied SOURCE projection before the first target write.
// The source repo is operator-controlled for forge-upgrade, so a symlink,
// hardlink, or non-regular leaf in the projection must fail closed.
export function assertSafeSourceProjection(sourceRoot, relPaths) {
  assertNoSymlinkComponents(sourceRoot, 'source directory');
  for (const rel of relPaths) {
    const source = join(sourceRoot, rel);
    assertNoSymlinkComponents(source, `source ${rel}`);
    regularFile(source, `source ${rel}`);
  }
  return relPaths;
}

const NOFOLLOW = constants.O_NOFOLLOW ?? 0;
const NONBLOCK = constants.O_NONBLOCK ?? 0;

function sameOpenedFile(pathStat, openedStat) {
  return pathStat.dev === openedStat.dev && pathStat.ino === openedStat.ino;
}

function openRegularNoFollow(path, flags, mode, label) {
  const allowMissing = Boolean(flags & constants.O_CREAT);
  inspectRegularFile(path, label, allowMissing);
  assertNoSymlinkComponents(path, label);
  let fd;
  try {
    // O_NONBLOCK is inert for regular files, but prevents a final-component FIFO
    // swap from hanging before fstat can reject it.
    fd = openSync(path, flags | NOFOLLOW | NONBLOCK, mode);
    // Re-check after open and before reading/writing. On POSIX O_NOFOLLOW makes
    // replacement of the final component atomic-fail; the second lexical check
    // narrows an ancestor-swap window on platforms without openat(dirfd).
    assertNoSymlinkComponents(path, label);
    const openedStat = validateRegularStat(fstatSync(fd), path, label);
    const pathStat = regularFile(path, label);
    if (!sameOpenedFile(pathStat, openedStat)) {
      throw new Error(`refusing changed file path for ${label}: ${path}`);
    }
    return fd;
  } catch (err) {
    if (fd !== undefined) closeSync(fd);
    if (err?.code === 'ELOOP') {
      throw new Error(`refusing unsafe symlink path for ${label}: ${path}`);
    }
    throw err;
  }
}

function sameReadState(left, right) {
  return ['dev', 'ino', 'mode', 'nlink', 'size', 'mtimeNs', 'ctimeNs']
    .every((field) => left[field] === right[field]);
}

function readStableDescriptor(fd, path, label, encoding = null, afterMetadata = null) {
  const before = validateRegularStat(fstatSync(fd, { bigint: true }), path, label);
  const beforePath = validateRegularStat(lstatSync(path, { bigint: true }), path, label);
  if (!sameReadState(before, beforePath)) {
    throw new Error(`refusing changed file path for ${label}: ${path}`);
  }
  afterMetadata?.();
  const value = encoding === null ? readFileSync(fd) : readFileSync(fd, encoding);
  const after = validateRegularStat(fstatSync(fd, { bigint: true }), path, label);
  const afterPath = validateRegularStat(lstatSync(path, { bigint: true }), path, label);
  if (!sameReadState(before, after) || !sameReadState(before, afterPath)) {
    throw new Error(`refusing changed file contents for ${label}: ${path}`);
  }
  return { stat: before, value };
}

export function readFileNoFollow(path, label = path, encoding = null) {
  const fd = openRegularNoFollow(path, constants.O_RDONLY, 0, label);
  try {
    return readStableDescriptor(fd, path, label, encoding).value;
  } finally {
    closeSync(fd);
  }
}

export function snapshotFileNoFollow(path, label = path, afterMetadata = null) {
  const fd = openRegularNoFollow(path, constants.O_RDONLY, 0, label);
  try {
    const { stat, value } = readStableDescriptor(fd, path, label, null, afterMetadata);
    return Object.freeze({
      bytes: value,
      identity: Object.freeze({ dev: stat.dev, ino: stat.ino }),
      mode: Number(stat.mode & 0o777n),
    });
  } finally { closeSync(fd); }
}

export function assertSafeRegularFile(path, label = path) {
  return regularFile(path, label);
}

// mkdir-recursive is preceded and followed by component checks. Existing leaves
// must be single-link regular files; the final file is opened with O_NOFOLLOW and
// O_NONBLOCK where the host exposes them, then truncated only after post-open
// inode/type/link checks. Node has no portable openat(dirfd), so an adversarial
// swap of an ancestor after the last check remains a documented platform boundary.
export function writeFileNoFollow(path, content, label = path, sourceMode = null) {
  const parent = dirname(path);
  assertNoSymlinkComponents(parent, `${label} parent`);
  assertNoSymlinkComponents(path, label);
  mkdirSync(parent, { recursive: true });
  assertNoSymlinkComponents(parent, `${label} parent`);
  assertNoSymlinkComponents(path, label);
  const existed = inspectRegularFile(path, label, true) !== null;
  const fd = openRegularNoFollow(
    path,
    constants.O_WRONLY | constants.O_CREAT,
    sourceMode ?? 0o666,
    label,
  );
  let written;
  try {
    ftruncateSync(fd, 0);
    writeFileSync(fd, content);
    if (!existed && sourceMode !== null && process.platform !== 'win32') {
      fchmodSync(fd, sourceMode & 0o777);
    }
    written = validateRegularStat(fstatSync(fd), path, label);
  } finally {
    closeSync(fd);
  }
  return Object.freeze({ dev: written.dev, ino: written.ino });
}

function restoreQuarantinedFile(
  quarantine, path, identity, previous, label,
) {
  const current = lstatSync(quarantine);
  if (!sameOpenedFile(identity, current)) {
    try {
      linkSync(quarantine, path);
      unlinkSync(quarantine);
    } catch (error) {
      throw new Error(
        `preserved replacement for ${label} at ${quarantine}: ${error.message}`,
      );
    }
    throw new Error(`preserved concurrent replacement for ${label}`);
  }
  if (previous === null) {
    unlinkSync(quarantine);
    return;
  }
  writeFileNoFollow(quarantine, previous, label);
  try {
    linkSync(quarantine, path);
  } catch (error) {
    throw new Error(`preserved prior ${label} at ${quarantine}: ${error.message}`);
  }
  unlinkSync(quarantine);
}

// Undo a completed write without checking one pathname and later unlinking it.
// rename(2) first captures whatever currently occupies the path inside a private
// directory; only a captured inode matching the write descriptor is removed or
// restored. A concurrent replacement is put back and never deleted.
export function rollbackWrittenFileNoFollow(
  path, identity, previous, label = path,
) {
  const parent = dirname(path);
  assertNoSymlinkComponents(parent, `${label} parent`);
  const directory = mkdtempSync(join(parent, '.forge-write-rollback-'));
  chmodSync(directory, 0o700);
  const quarantine = join(directory, 'written');
  let failure = null;
  try {
    try {
      renameSync(path, quarantine);
    } catch (error) {
      if (error?.code === 'ENOENT' && previous === null) return;
      throw error;
    }
    restoreQuarantinedFile(quarantine, path, identity, previous, label);
  } catch (error) {
    failure = error;
    throw error;
  } finally {
    try {
      rmdirSync(directory);
    } catch (error) {
      if (failure === null && error?.code !== 'ENOENT') throw error;
    }
  }
}

function cleanupExclusiveStage(fd, temporary, directory) {
  let cleanupError = null;
  if (fd !== undefined) {
    try { closeSync(fd); } catch (error) { cleanupError ??= error; }
  }
  try { unlinkSync(temporary); } catch (error) {
    if (error?.code !== 'ENOENT') cleanupError ??= error;
  }
  try { rmdirSync(directory); } catch (error) {
    if (error?.code !== 'ENOENT') cleanupError ??= error;
  }
  return cleanupError;
}

function exclusiveStage(parent, mode, label) {
  const directory = mkdtempSync(join(parent, '.forge-exclusive-'));
  chmodSync(directory, 0o700);
  const temporary = join(directory, 'claim');
  let fd;
  try {
    fd = openSync(
      temporary,
      constants.O_RDWR | constants.O_CREAT | constants.O_EXCL | NOFOLLOW | NONBLOCK,
      mode,
    );
    const opened = validateRegularStat(
      fstatSync(fd, { bigint: true }), temporary, label,
    );
    const lexical = validateRegularStat(
      lstatSync(temporary, { bigint: true }), temporary, label,
    );
    if (!sameOpenedFile(opened, lexical)) {
      throw new Error(`refusing changed exclusive stage for ${label}`);
    }
    return { directory, fd, temporary };
  } catch (error) {
    cleanupExclusiveStage(fd, temporary, directory);
    throw error;
  }
}

function publishExclusive(path, content, label, sourceMode, anchored = false) {
  const parent = dirname(path);
  const stage = exclusiveStage(parent, sourceMode ?? 0o666, label);
  let created;
  try {
    writeFileSync(stage.fd, content);
    if (sourceMode !== null && process.platform !== 'win32') {
      fchmodSync(stage.fd, sourceMode & 0o777);
    }
    created = validateRegularStat(
      fstatSync(stage.fd, { bigint: true }), stage.temporary, label,
    );
  } catch (error) {
    const cleanupError = cleanupExclusiveStage(
      stage.fd, stage.temporary, stage.directory);
    if (cleanupError) {
      throw new Error(`${error.message}; exclusive staging cleanup failed: ${cleanupError.message}`);
    }
    throw error;
  }
  try {
    linkSync(stage.temporary, path);
  } catch (error) {
    const cleanupError = cleanupExclusiveStage(
      stage.fd, stage.temporary, stage.directory);
    if (cleanupError) {
      throw new Error(`${error.message}; exclusive staging cleanup failed: ${cleanupError.message}`);
    }
    if (error?.code !== 'EEXIST') throw error;
    if (anchored) validateRegularStat(lstatSync(path), path, label);
    else readFileNoFollow(path, label);
    return null;
  }
  return Object.freeze({
    directory: stage.directory,
    fd: stage.fd,
    identity: Object.freeze({ dev: created.dev, ino: created.ino }),
    sentinel: stage.temporary,
  });
}

// Write into a private same-directory inode before atomically publishing it by
// hard link. The returned claim retains the descriptor and private sentinel so
// the caller owns rollback metadata before any post-publication cleanup.
export function writeFileExclusiveNoFollow(
  path, content, label = path, sourceMode = null,
) {
  const parent = dirname(path);
  assertNoSymlinkComponents(parent, `${label} parent`);
  assertNoSymlinkComponents(path, label);
  mkdirSync(parent, { recursive: true });
  assertNoSymlinkComponents(parent, `${label} parent`);
  assertNoSymlinkComponents(path, label);
  return publishExclusive(path, content, label, sourceMode);
}

export function writeFileExclusiveAnchoredNoFollow(
  path, content, label = path, sourceMode = null,
) {
  return publishExclusive(path, content, label, sourceMode, true);
}

export function releaseFileExclusiveClaim(claim, label = 'exclusive file') {
  const errors = [];
  try { unlinkSync(claim.sentinel); } catch (error) {
    if (error?.code !== 'ENOENT') errors.push(error);
  }
  try { closeSync(claim.fd); } catch (error) { errors.push(error); }
  try { rmdirSync(claim.directory); } catch (error) {
    if (error?.code !== 'ENOENT') errors.push(error);
  }
  if (errors.length > 0) {
    throw new Error(
      `${label} committed but claim cleanup failed; recovery directory ` +
      `${claim.directory}: ${errors.map((error) => error.message).join('; ')}`,
    );
  }
}

export function copyFileNoFollow(source, destination, sourceLabel, destinationLabel) {
  const snapshot = snapshotFileNoFollow(source, sourceLabel);
  writeFileNoFollow(destination, snapshot.bytes, destinationLabel, snapshot.mode);
}

export function copyFileExclusiveNoFollow(
  source, destination, sourceLabel, destinationLabel,
) {
  const snapshot = snapshotFileNoFollow(source, sourceLabel);
  return writeFileExclusiveNoFollow(
    destination, snapshot.bytes, destinationLabel, snapshot.mode,
  );
}

export function copyFileExclusiveAnchoredNoFollow(
  source, destination, sourceLabel, destinationLabel,
) {
  const snapshot = snapshotFileNoFollow(source, sourceLabel);
  return writeFileExclusiveAnchoredNoFollow(
    destination, snapshot.bytes, destinationLabel, snapshot.mode,
  );
}

// Copy one file from <sourceRoot>/<relPath> into <targetDir>/<relPath>, creating
// parent dirs. Pushes relPath onto `created` so the caller can report what landed.
export function copyFromSource(relPath, sourceRoot, targetDir, created) {
  const source = join(sourceRoot, relPath);
  const dest = join(targetDir, relPath);
  copyFileNoFollow(source, dest, `source ${relPath}`, relPath);
  created.push(relPath);
}

// Recursively copy a whole SOURCE directory tree into the target (verbatim),
// preserving structure. Used for the .agent governance-asset dirs. Skips Python
// bytecode caches so a generated project ships clean source only. The __pycache__
// skip here is THE rule enumerateTree mirrors — keep them in lockstep.
export function copyTree(relDir, sourceRoot, targetDir, created) {
  const srcDir = join(sourceRoot, relDir);
  assertNoSymlinkComponents(srcDir, `source ${relDir}`);
  let rootStat;
  try {
    rootStat = lstatSync(srcDir);
  } catch (err) {
    throw new Error(`cannot safely inspect source ${relDir}: ${err.message}`);
  }
  if (!rootStat.isDirectory()) {
    throw new Error(`refusing unsafe non-directory source ${relDir}: ${srcDir}`);
  }
  for (const entry of readdirSync(srcDir, { withFileTypes: true })) {
    const childRel = join(relDir, entry.name);
    const child = join(sourceRoot, childRel);
    assertNoSymlinkComponents(child, `source ${childRel}`);
    const st = regularFileOrDirectory(child, `source ${childRel}`);
    if (entry.name === '__pycache__') continue;
    if (st.isDirectory()) copyTree(childRel, sourceRoot, targetDir, created);
    else copyFromSource(childRel, sourceRoot, targetDir, created);
  }
}

function regularFileOrDirectory(path, label) {
  let st;
  try {
    st = lstatSync(path);
  } catch (err) {
    throw new Error(`cannot safely inspect ${label}: ${err.message}`);
  }
  if (st.isSymbolicLink()) {
    throw new Error(`refusing unsafe symlink path for ${label}: ${path}`);
  }
  if (!st.isFile() && !st.isDirectory()) {
    throw new Error(`refusing unsafe special path for ${label}: ${path}`);
  }
  if (st.isFile() && Number(st.nlink) > 1) {
    throw new Error(`refusing unsafe hardlink path for ${label}: ${path}`);
  }
  return st;
}

// Read-only twin of copyTree; keep its __pycache__ rule identical.
export function enumerateTree(relDir, sourceRoot) {
  const out = [];
  const srcDir = join(sourceRoot, relDir);
  assertNoSymlinkComponents(srcDir, `source ${relDir}`);
  const rootStat = regularFileOrDirectory(srcDir, `source ${relDir}`);
  if (!rootStat.isDirectory()) {
    throw new Error(`refusing unsafe non-directory source ${relDir}: ${srcDir}`);
  }
  for (const entry of readdirSync(srcDir, { withFileTypes: true })) {
    const childRel = join(relDir, entry.name);
    const child = join(sourceRoot, childRel);
    assertNoSymlinkComponents(child, `source ${childRel}`);
    const st = regularFileOrDirectory(child, `source ${childRel}`);
    if (entry.name === '__pycache__') continue;
    if (st.isDirectory()) out.push(...enumerateTree(childRel, sourceRoot));
    else out.push(childRel);
  }
  return out;
}
