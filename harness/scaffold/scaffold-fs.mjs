// ForgeOS scaffold-fs — the shared SOURCE-tree copy/enumeration primitives used
// by BOTH forge-init (scaffold a new project) and forge-upgrade (resync an
// existing project's copied governance). Extracted so the copy semantics live in
// ONE place: the recursive __pycache__-skipping walk that decides what a copied
// governance-asset tree contains is a SINGLE source of truth, not duplicated
// between the scaffolder and the upgrader (where they could silently drift —
// upgrade enumerating a tree differently than scaffold copied it would mean a
// project that can never reach byte-identical-to-source).
//
// Four primitives over a `sourceRoot` (the ForgeOS SOURCE repo) and a `targetDir`:
//   * copyFromSource(rel, ...)  — copy one file, creating parent dirs.
//   * copyFileExclusiveNoFollow(...) — seed once without truncating a race winner.
//   * copyTree(relDir, ...)     — recursively copy a whole dir (skips __pycache__).
//   * enumerateTree(relDir, sourceRoot) -> rel[]  — the PURE projection of what
//       copyTree WOULD copy (same __pycache__ skip rule), with NO writes. upgrade
//       expands GOVERNANCE_DIRS through this to know every file a tree contributes.
//
// Zero third-party deps (node: builtins only). This is a SCAFFOLD/UPGRADE-time
// tool, not project runtime governance, so it is on forge-init's HARNESS_NOT_COPIED
// whitelist (a generated project does not scaffold sub-projects).
import {
  closeSync,
  constants,
  fchmodSync,
  fstatSync,
  ftruncateSync,
  lstatSync,
  mkdirSync,
  openSync,
  readFileSync,
  readdirSync,
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
  const parsed = parse(absolute);
  let cursor = parsed.root;
  const parts = absolute.slice(parsed.root.length).split(/[\\/]/).filter(Boolean);
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

export function readFileNoFollow(path, label = path, encoding = null) {
  const fd = openRegularNoFollow(path, constants.O_RDONLY, 0, label);
  try {
    return encoding === null ? readFileSync(fd) : readFileSync(fd, encoding);
  } finally {
    closeSync(fd);
  }
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
  try {
    ftruncateSync(fd, 0);
    writeFileSync(fd, content);
    if (!existed && sourceMode !== null && process.platform !== 'win32') {
      fchmodSync(fd, sourceMode & 0o777);
    }
  } finally {
    closeSync(fd);
  }
}

// Seed a project-owned file exactly once. O_EXCL closes the gap between an
// earlier "missing" observation and this write: if another process creates the
// file first, validate that leaf through the same no-follow path and preserve
// its bytes. Existing files are never truncated by this primitive.
export function writeFileExclusiveNoFollow(
  path, content, label = path, sourceMode = null,
) {
  const parent = dirname(path);
  assertNoSymlinkComponents(parent, `${label} parent`);
  assertNoSymlinkComponents(path, label);
  mkdirSync(parent, { recursive: true });
  assertNoSymlinkComponents(parent, `${label} parent`);
  assertNoSymlinkComponents(path, label);
  let fd;
  try {
    fd = openRegularNoFollow(
      path,
      constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL,
      sourceMode ?? 0o666,
      label,
    );
  } catch (err) {
    if (err?.code !== 'EEXIST') throw err;
    readFileNoFollow(path, label);
    return false;
  }
  try {
    writeFileSync(fd, content);
    if (sourceMode !== null && process.platform !== 'win32') {
      fchmodSync(fd, sourceMode & 0o777);
    }
  } finally {
    closeSync(fd);
  }
  return true;
}

export function copyFileNoFollow(source, destination, sourceLabel, destinationLabel) {
  const sourceStat = regularFile(source, sourceLabel);
  const content = readFileNoFollow(source, sourceLabel);
  writeFileNoFollow(destination, content, destinationLabel, sourceStat.mode);
}

export function copyFileExclusiveNoFollow(
  source, destination, sourceLabel, destinationLabel,
) {
  const sourceStat = regularFile(source, sourceLabel);
  const content = readFileNoFollow(source, sourceLabel);
  return writeFileExclusiveNoFollow(
    destination, content, destinationLabel, sourceStat.mode,
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

// enumerateTree(relDir, sourceRoot) -> array of relative paths copyTree WOULD copy
// from <sourceRoot>/<relDir>, in directory order, applying the SAME __pycache__
// skip. PURE-ish: it reads the source dir structure but writes NOTHING — the
// read-only twin of copyTree, so forge-upgrade can project GOVERNANCE_DIRS into
// concrete files (to byte-compare each against the target) using the exact set
// the scaffolder would have produced. Single source of truth for "what is in a
// copied governance tree" — if copyTree's skip rule changes, change it once here.
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
