// Explicitly advisory acceptance cache. Formal `forge accept`, `--json`, and
// the synchronous collect() path never consult this module. A cache hit is only
// an edit-time acceleration hint and is always labelled as such by the CLI.
import { createHash, randomUUID } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import {
  closeSync, constants as fsConstants, existsSync, fstatSync, fsyncSync,
  lstatSync, mkdirSync, openSync, readSync, renameSync, unlinkSync, writeFileSync,
} from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { candidateFingerprint } from './acceptance-candidate.mjs';

export const HARNESS_DIR = dirname(fileURLToPath(import.meta.url));
export const ROOT = dirname(HARNESS_DIR);
export const CACHE_SCHEMA = 6;

const CACHE_FILE = 'acceptance-cache.json';
const CACHE_MAX_BYTES = 1024 * 1024;
const DETAIL_MAX_BYTES = 256 * 1024;
const HASH_RE = /^[a-f0-9]{64}$/;
const STORE_FIELDS = new Set(['schema', 'rows']);
const STORED_ROW_FIELDS = new Set(['fingerprint', 'status', 'detail', 'category']);
const STATUSES = new Set(['PASS', 'FAIL', 'N-A']);
const CATEGORIES = new Set(['applicable', 'inapplicable', 'no_tool']);
const PYTHON_TOOL_IDENTITY = String.raw`
import hashlib, importlib.util, pathlib
names = ("yaml", "pytest", "pytest_cov", "coverage")
digest = hashlib.sha256()
parts = []
for name in names:
    spec = importlib.util.find_spec(name)
    origin = pathlib.Path(spec.origin).resolve() if spec and spec.origin else None
    root = origin.parent if origin and origin.is_file() else origin
    version = "unknown"
    if root and root.is_dir():
        for path in sorted(root.rglob("*")):
            if path.is_file() and "__pycache__" not in path.parts:
                digest.update(str(path.relative_to(root)).encode())
                digest.update(path.read_bytes())
    parts.append((name, str(origin), version))
print(parts, digest.hexdigest())
`;

// Advisory replay is limited to source-only scanners whose complete inputs are
// represented by the candidate plus the pinned runtimes below. Execution rows
// stay live: node_modules, language package caches, compiler caches, and other
// installed dependency trees are outside the source candidate, so replaying a
// test/build/lint result would claim an equivalence the fingerprint cannot prove.
// SCA also stays live because an external advisory DB may be outside the tree.
export const CACHEABLE_PLANS = Object.freeze(Object.assign(Object.create(null), {
  complexity_violations: { criterion: 'complexity_violations' },
  arch_violations: { criterion: 'arch_violations' },
  architecture: { criterion: 'architecture' },
  security_findings: { criterion: 'security_findings' },
}));
export const PROBE_INPUTS = Object.freeze(Object.fromEntries(
  Object.keys(CACHEABLE_PLANS).map((name) => [name, { scope: 'repository-source' }]),
));

const TOOL_PROBES = [
  ['node', ['--version']], ['python3', ['--version']], ['go', ['version']],
  ['python3', ['-B', '-c', PYTHON_TOOL_IDENTITY]],
  ['rustc', ['--version', '--verbose']], ['cargo', ['--version', '--verbose']],
  ['git', ['--version']], ['npm', ['--version']], ['java', ['-version']],
  ['javac', ['-version']],
];

function hasCacheablePlan(name) {
  return Object.hasOwn(CACHEABLE_PLANS, name);
}

function emptyStore() {
  return { schema: CACHE_SCHEMA, rows: {} };
}

function toolIdentity(root, env) {
  return TOOL_PROBES.map(([command, args]) => {
    const run = spawnSync(command, args, {
      cwd: root, env, encoding: 'utf8', timeout: 10_000, maxBuffer: 64 * 1024,
    });
    const output = `${run.stdout ?? ''}${run.stderr ?? ''}`.trim().slice(0, 2048);
    return JSON.stringify({ command, args, status: run.status, error: run.error?.code ?? null, output });
  });
}

function candidateBaseFingerprint(root, env) {
  const effective = {
    ...env, FORGE_ACCEPT_NO_CACHE: '1', PYTHONDONTWRITEBYTECODE: '1',
  };
  const hash = createHash('sha256').update(`forge-advisory-cache/${CACHE_SCHEMA}\0`);
  hash.update(`candidate\0${candidateFingerprint(root, effective)}\0`);
  hash.update(`runtime\0${process.execPath}\0${JSON.stringify(process.versions)}\0`);
  for (const identity of toolIdentity(root, effective)) hash.update(`tool\0${identity}\0`);
  for (const key of Object.keys(effective).sort()) {
    hash.update(`env\0${key}\0${effective[key]}\0`);
  }
  return hash.digest('hex');
}

export function fingerprintProbes(names, root = ROOT, env = process.env) {
  for (const name of names) {
    if (!hasCacheablePlan(name)) throw new Error(`acceptance-cache: unknown probe ${name}`);
  }
  const base = candidateBaseFingerprint(root, env);
  return new Map(names.map((name) => [
    name, createHash('sha256').update(`${base}\0${name}`).digest('hex'),
  ]));
}

export function fingerprintProbe(name, root = ROOT, env = process.env) {
  return fingerprintProbes([name], root, env).get(name);
}

export function storePath(root = ROOT) {
  return join(root, '.forge', CACHE_FILE);
}

function ownedByProcess(info) {
  return typeof process.getuid !== 'function' || info.uid === process.getuid();
}

function privateMode(info) {
  return (info.mode & 0o077) === 0;
}

function secureCacheDirectory(root, create = false) {
  const path = join(root, '.forge');
  if (!existsSync(path) && create) mkdirSync(path, { mode: 0o700 });
  if (!Number.isInteger(fsConstants.O_DIRECTORY) || !Number.isInteger(fsConstants.O_NOFOLLOW)
      || process.platform !== 'linux') {
    throw new Error('acceptance-cache: descriptor-anchored private storage unavailable');
  }
  const before = lstatSync(path);
  const descriptor = openSync(
    path, fsConstants.O_RDONLY | fsConstants.O_DIRECTORY | fsConstants.O_NOFOLLOW,
  );
  const info = fstatSync(descriptor);
  if (before.dev !== info.dev || before.ino !== info.ino || !info.isDirectory()
      || !ownedByProcess(info) || !privateMode(info)) {
    closeSync(descriptor);
    throw new Error(`acceptance-cache: unsafe private directory ${path}`);
  }
  const anchor = `/proc/self/fd/${descriptor}`;
  if (!existsSync(anchor)) {
    closeSync(descriptor);
    throw new Error('acceptance-cache: private directory descriptor is not addressable');
  }
  return { anchor, descriptor, info, path };
}

function readSecureCacheFile(root) {
  if (!existsSync(join(root, '.forge'))) return null;
  const directory = secureCacheDirectory(root);
  const path = join(directory.anchor, CACHE_FILE);
  try {
    if (!existsSync(path)) return null;
    const before = lstatSync(path);
    if (before.isSymbolicLink() || !before.isFile() || before.nlink !== 1
        || !ownedByProcess(before) || !privateMode(before)
        || before.size <= 0 || before.size > CACHE_MAX_BYTES) {
      throw new Error(`acceptance-cache: unsafe cache file ${storePath(root)}`);
    }
    const fd = openSync(path, fsConstants.O_RDONLY | fsConstants.O_NOFOLLOW);
    try {
      const actual = fstatSync(fd);
      if (actual.dev !== before.dev || actual.ino !== before.ino || actual.size !== before.size) {
        throw new Error(`acceptance-cache: cache file raced open ${storePath(root)}`);
      }
      const bytes = Buffer.alloc(actual.size);
      let offset = 0;
      while (offset < bytes.length) {
        const count = readSync(fd, bytes, offset, bytes.length - offset, null);
        if (count <= 0) throw new Error('acceptance-cache: cache file ended early');
        offset += count;
      }
      const after = fstatSync(fd);
      if (after.dev !== actual.dev || after.ino !== actual.ino || after.size !== actual.size
          || after.mtimeMs !== actual.mtimeMs || after.ctimeMs !== actual.ctimeMs) {
        throw new Error('acceptance-cache: cache file changed while read');
      }
      return bytes.toString('utf8');
    } finally { closeSync(fd); }
  } finally {
    closeSync(directory.descriptor);
  }
}

function validStoredRow(name, row) {
  if (!hasCacheablePlan(name) || !row || Array.isArray(row) || typeof row !== 'object') return false;
  const fields = Object.keys(row);
  if (fields.length !== STORED_ROW_FIELDS.size
      || fields.some((field) => !STORED_ROW_FIELDS.has(field))) return false;
  if (!HASH_RE.test(row.fingerprint) || !STATUSES.has(row.status)
      || typeof row.detail !== 'string' || Buffer.byteLength(row.detail) > DETAIL_MAX_BYTES
      || !CATEGORIES.has(row.category)) return false;
  if (row.status === 'N-A') return row.category !== 'applicable';
  return row.category === 'applicable';
}

function parseStore(text) {
  const value = JSON.parse(text);
  if (`${JSON.stringify(value)}\n` !== text) return emptyStore();
  const fields = value && !Array.isArray(value) && typeof value === 'object'
    ? Object.keys(value) : [];
  if (!value || Array.isArray(value) || value.schema !== CACHE_SCHEMA
      || fields.length !== STORE_FIELDS.size || fields.some((field) => !STORE_FIELDS.has(field))
      || !value.rows || Array.isArray(value.rows) || typeof value.rows !== 'object') {
    return emptyStore();
  }
  const entries = Object.entries(value.rows);
  if (entries.length !== Object.keys(CACHEABLE_PLANS).length) return emptyStore();
  const rows = {};
  for (const [name, row] of entries) {
    if (!validStoredRow(name, row)) return emptyStore();
    rows[name] = row;
  }
  return { schema: CACHE_SCHEMA, rows };
}

export function loadCache(root = ROOT) {
  try {
    const text = readSecureCacheFile(root);
    return text === null ? emptyStore() : parseStore(text);
  } catch {
    return emptyStore();
  }
}

export function cachedDecide(name, criterion, root = ROOT, fingerprint, store) {
  if (!hasCacheablePlan(name)) return null;
  const expected = fingerprint ?? fingerprintProbe(name, root);
  const row = (store ?? loadCache(root)).rows[name];
  if (!row || row.fingerprint !== expected
      || CACHEABLE_PLANS[name].criterion !== criterion) return null;
  return {
    criterion, status: row.status, category: row.category,
    detail: `${row.detail} [advisory cache]`,
  };
}

function assertWritableTarget(directory, root) {
  const path = join(directory.anchor, CACHE_FILE);
  let info;
  try { info = lstatSync(path); }
  catch (error) {
    if (error?.code === 'ENOENT') return;
    throw error;
  }
  if (info.isSymbolicLink() || !info.isFile() || info.nlink !== 1
      || !ownedByProcess(info) || !privateMode(info)) {
    throw new Error(`acceptance-cache: refusing unsafe cache target ${storePath(root)}`);
  }
}

function writePrivateTemporary(directory, body) {
  const name = `.acceptance-cache-${process.pid}-${randomUUID()}.tmp`;
  const temporary = join(directory.anchor, name);
  const flags = fsConstants.O_WRONLY | fsConstants.O_CREAT | fsConstants.O_EXCL | fsConstants.O_NOFOLLOW;
  const fd = openSync(temporary, flags, 0o600);
  let failure = null;
  try {
    writeFileSync(fd, body, 'utf8');
    fsyncSync(fd);
  } catch (error) {
    failure = error;
  } finally {
    try { closeSync(fd); } catch (error) { failure ??= error; }
  }
  if (!failure) return temporary;
  try { unlinkSync(temporary); } catch { /* best-effort private temp cleanup */ }
  throw failure;
}

function writeStoreAtomically(root, store) {
  const directory = secureCacheDirectory(root, true);
  try {
    assertWritableTarget(directory, root);
    const path = join(directory.anchor, CACHE_FILE);
    const body = `${JSON.stringify(store)}\n`;
    if (Buffer.byteLength(body) > CACHE_MAX_BYTES) {
      throw new Error('acceptance-cache: store exceeds size cap');
    }
    const temporary = writePrivateTemporary(directory, body);
    try {
      const currentDir = lstatSync(directory.path);
      if (currentDir.dev !== directory.info.dev || currentDir.ino !== directory.info.ino) {
        throw new Error('acceptance-cache: private directory changed before publish');
      }
      renameSync(temporary, path);
    } catch (error) {
      try { unlinkSync(temporary); } catch { /* best-effort private temp cleanup */ }
      throw error;
    }
  } finally {
    closeSync(directory.descriptor);
  }
}

export function saveRows(root, entries) {
  try {
    const store = emptyStore();
    const expected = Object.keys(CACHEABLE_PLANS).length;
    if (!Array.isArray(entries) || entries.length !== expected) {
      throw new Error('cache snapshot must contain every cacheable row exactly once');
    }
    const seen = new Set();
    for (const entry of entries) {
      if (seen.has(entry.name)) throw new Error(`duplicate cache row ${entry.name}`);
      seen.add(entry.name);
      const row = {
        fingerprint: entry.fingerprint, status: entry.result?.status,
        detail: entry.result?.detail, category: entry.result?.category,
      };
      if (!validStoredRow(entry.name, row)) throw new Error(`invalid cache row ${entry.name}`);
      store.rows[entry.name] = row;
    }
    writeStoreAtomically(root, store);
    return true;
  } catch {
    return false;
  }
}

export function cacheDisabled(env = process.env) {
  return env.FORGE_ACCEPT_NO_CACHE === '1';
}
