// Linux continuous mutation journal for formal acceptance. A separate helper
// drains raw inotify events while the coordinator performs synchronous hashes.
// Scope: trusted local allowlisted filesystems and ordinary inotify-visible
// path operations. mmap writes, privileged mount changes, and remote filesystems
// are outside this proof boundary. A barrier linearizes at the helper's drain;
// close only releases resources and does not extend the accepted interval.
import { randomUUID } from 'node:crypto';
import { spawn } from 'node:child_process';
import {
  closeSync, constants as fsConstants, existsSync, fstatSync, lstatSync,
  mkdirSync, openSync, readFileSync, readSync, realpathSync, statfsSync, unlinkSync,
} from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HELPER = fileURLToPath(new URL('./candidate-journal.py', import.meta.url));
const PYTHON = '/usr/bin/python3';
const READY_TIMEOUT_MS = 15_000;
const BARRIER_TIMEOUT_MS = 5_000;
const CLOSE_GRACE_MS = 500;
const CLOSE_TIMEOUT_MS = 5_000;
const MAX_LINE_BYTES = 256 * 1024;
const MAX_OUTPUT_BYTES = 1024 * 1024;
const MAX_STDERR_BYTES = 16 * 1024;
const MAX_EVENTS = 32;
const MAX_HELPER_BYTES = 256 * 1024;
const MAX_PENDING = 8;
const GENERATED_DIRS = [
  '.git', '.forge', 'node_modules', 'target', 'coverage', 'dist', 'build',
  '.next', '__pycache__', '.coverage', 'coverage.json', 'coverage.out',
];
const GENERATED_PREFIXES = ['.forge-coverage-'];
const LOCAL_FILESYSTEMS = new Set([
  0xef53n, 0x58465342n, 0x9123683en, 0x01021994n, 0x794c7630n,
]);

function pause(milliseconds) {
  const cell = new Int32Array(new SharedArrayBuffer(4));
  Atomics.wait(cell, 0, 0, milliseconds);
}

function descriptorAnchor(descriptor) {
  for (const base of ['/proc/self/fd', '/dev/fd']) {
    const anchor = join(base, String(descriptor));
    if (existsSync(anchor)) return anchor;
  }
  throw new Error('candidate journal directory descriptor is not addressable');
}

function validateHost(root) {
  if (process.platform !== 'linux') {
    throw new Error('continuous candidate journaling requires Linux inotify');
  }
  const rootInfo = lstatSync(root);
  if (!rootInfo.isDirectory() || rootInfo.isSymbolicLink()) {
    throw new Error('candidate journal root must be a real directory');
  }
  if (root === '/') throw new Error('candidate journal cannot monitor the filesystem root');
  for (const path of ancestorPaths(root)) {
    const filesystem = statfsSync(path, { bigint: true });
    if (!LOCAL_FILESYSTEMS.has(filesystem.type)) {
      throw new Error(`candidate journal filesystem is unsupported: 0x${filesystem.type.toString(16)}`);
    }
  }
}

function ancestorPaths(root) {
  const paths = [];
  for (let path = root; ; path = dirname(path)) {
    paths.push(path);
    if (dirname(path) === path) return paths;
  }
}

function trustedRootOwner(uid, uidMapText) {
  if (!Number.isSafeInteger(uid)) return false;
  const lines = String(uidMapText).trim().split('\n');
  if (lines.length === 0 || lines[0] === '') return false;
  const mappings = [];
  for (const line of lines) {
    const fields = line.trim().split(/\s+/).map(Number);
    if (fields.length !== 3 || fields.some((value) => (
      !Number.isSafeInteger(value) || value < 0
    )) || fields[2] < 1) return false;
    mappings.push(fields);
  }
  const initial = mappings.length === 1
    && mappings[0][0] === 0 && mappings[0][1] === 0
    && mappings[0][2] === 4_294_967_295;
  return initial && uid === 0;
}

function trustedPythonDescriptor() {
  let descriptor;
  try {
    const resolved = realpathSync(PYTHON);
    const before = lstatSync(resolved);
    descriptor = openSync(resolved, fsConstants.O_RDONLY | fsConstants.O_NOFOLLOW);
    const opened = fstatSync(descriptor);
    const uidMap = readFileSync('/proc/self/uid_map', 'utf8');
    const sameFile = before.dev === opened.dev && before.ino === opened.ino;
    if (!sameFile || !opened.isFile() || (opened.mode & 0o111) === 0
        || (opened.mode & 0o022) !== 0
        || !trustedRootOwner(opened.uid, uidMap)) {
      throw new Error('unsafe interpreter identity');
    }
    return descriptor;
  } catch (error) {
    if (descriptor !== undefined) closeSync(descriptor);
    throw new Error(`candidate journal Python interpreter is unsafe (${error.message})`);
  }
}

function sameSnapshot(first, second) {
  return first.dev === second.dev && first.ino === second.ino
    && first.mode === second.mode && first.size === second.size
    && first.mtimeNs === second.mtimeNs && first.ctimeNs === second.ctimeNs;
}

function immutableHelperSource() {
  let descriptor;
  try {
    const before = lstatSync(HELPER, { bigint: true });
    descriptor = openSync(HELPER, fsConstants.O_RDONLY | fsConstants.O_NOFOLLOW);
    const opened = fstatSync(descriptor, { bigint: true });
    if (!opened.isFile() || opened.size < 1n || opened.size > BigInt(MAX_HELPER_BYTES)
        || !sameSnapshot(before, opened)) throw new Error('unsafe helper identity');
    const bytes = readFileSync(descriptor);
    const afterOpen = fstatSync(descriptor, { bigint: true });
    const afterPath = lstatSync(HELPER, { bigint: true });
    if (BigInt(bytes.length) !== opened.size || !sameSnapshot(opened, afterOpen)
        || !sameSnapshot(opened, afterPath)) throw new Error('helper changed while pinned');
    const source = bytes.toString('utf8');
    if (!Buffer.from(source).equals(bytes) || source.includes('\0')) {
      throw new Error('helper source is not canonical UTF-8');
    }
    return source;
  } finally {
    if (descriptor !== undefined) closeSync(descriptor);
  }
}

function openControlDirectory(root) {
  const path = join(root, '.forge');
  let descriptor;
  try { mkdirSync(path, { mode: 0o700 }); }
  catch (error) { if (error?.code !== 'EEXIST') throw error; }
  try {
    const before = lstatSync(path);
    descriptor = openSync(
      path, fsConstants.O_RDONLY | fsConstants.O_DIRECTORY | fsConstants.O_NOFOLLOW,
    );
    const opened = fstatSync(descriptor);
    const after = lstatSync(path);
    const owned = typeof process.getuid !== 'function' || opened.uid === process.getuid();
    if (!opened.isDirectory() || !owned || (opened.mode & 0o077) !== 0
        || before.dev !== opened.dev || before.ino !== opened.ino
        || after.dev !== opened.dev || after.ino !== opened.ino) {
      throw new Error('candidate journal control directory is unsafe');
    }
    return { anchor: descriptorAnchor(descriptor), descriptor };
  } catch (error) {
    if (descriptor !== undefined) closeSync(descriptor);
    throw error;
  }
}

function openReadyFile(directory) {
  const name = `acceptance-journal-${randomUUID()}.ready`;
  const path = join(directory.anchor, name);
  const descriptor = openSync(
    path, fsConstants.O_RDWR | fsConstants.O_CREAT | fsConstants.O_EXCL
      | fsConstants.O_NOFOLLOW,
    0o600,
  );
  return { descriptor, name, path };
}

function cleanupReady(directory, ready) {
  try { if (ready) unlinkSync(ready.path); } catch { /* already absent */ }
  try { if (ready) closeSync(ready.descriptor); } catch { /* already closed */ }
  try { closeSync(directory.descriptor); } catch { /* already closed */ }
}

function parseReady(text) {
  let value;
  try { value = JSON.parse(text); }
  catch { throw new Error('candidate journal helper emitted invalid readiness JSON'); }
  if (JSON.stringify(value) !== text
      || JSON.stringify(Object.keys(value)) !== JSON.stringify(['ok', 'error'])
      || typeof value.ok !== 'boolean'
      || (value.error !== null && typeof value.error !== 'string')) {
    throw new Error('candidate journal helper emitted an invalid readiness record');
  }
  if (!value.ok) throw new Error(value.error || 'candidate journal helper setup failed');
}

function waitForReady(ready) {
  const deadline = Date.now() + READY_TIMEOUT_MS;
  while (Date.now() < deadline) {
    const size = Number(fstatSync(ready.descriptor).size);
    if (size > MAX_LINE_BYTES) throw new Error('candidate journal readiness output exceeded limit');
    if (size > 0) {
      const buffer = Buffer.alloc(size);
      if (readSync(ready.descriptor, buffer, 0, size, 0) !== size) {
        throw new Error('candidate journal readiness output was truncated');
      }
      const text = buffer.toString('utf8');
      if (!Buffer.from(text).equals(buffer) || !text.endsWith('\n')) {
        throw new Error('candidate journal readiness output is not UTF-8 framed');
      }
      parseReady(text.slice(0, -1));
      return;
    }
    pause(10);
  }
  throw new Error(`candidate journal helper setup timed out after ${READY_TIMEOUT_MS}ms`);
}

function markFailure(state, error) {
  if (!state.error) state.error = error instanceof Error ? error : new Error(String(error));
  rejectPending(state, state.error);
}

function rejectPending(state, error) {
  for (const pending of state.pending.values()) {
    clearTimeout(pending.timer);
    pending.reject(error);
  }
  state.pending.clear();
}

function stopHelper(state) {
  if (state.child.exitCode === null && state.child.signalCode === null) {
    try { state.child.kill('SIGKILL'); } catch { /* already gone */ }
  }
}

function protocolFailure(state, message) {
  markFailure(state, new Error(message));
  stopHelper(state);
}

function validEvent(path) {
  const generated = (part) => GENERATED_DIRS.includes(part)
    || GENERATED_PREFIXES.some((prefix) => part.startsWith(prefix));
  return typeof path === 'string' && Buffer.byteLength(path) <= 4096
    && !path.includes('\0') && path !== ''
    && !path.replaceAll('\\', '/').split('/').some(generated);
}

function validateReply(text) {
  let value;
  try { value = JSON.parse(text); }
  catch { throw new Error('candidate journal helper emitted invalid JSON'); }
  const fields = ['id', 'op', 'ok', 'dirty', 'overflow', 'events', 'error'];
  if (JSON.stringify(value) !== text
      || JSON.stringify(Object.keys(value)) !== JSON.stringify(fields)
      || !Number.isSafeInteger(value.id) || value.id < 1 || value.op !== 'BARRIER'
      || typeof value.ok !== 'boolean' || typeof value.dirty !== 'boolean'
      || typeof value.overflow !== 'boolean' || !Array.isArray(value.events)
      || value.events.length > MAX_EVENTS || !value.events.every(validEvent)
      || (value.error !== null && (typeof value.error !== 'string'
        || Buffer.byteLength(value.error) > 4096))
      || value.ok !== (value.error === null)
      || value.dirty !== (value.events.length > 0
        || value.overflow || value.error !== null)) {
    throw new Error('candidate journal helper emitted an invalid protocol record');
  }
  return value;
}

function acceptReply(state, reply) {
  if (state.closed) return;
  const pending = state.pending.get(reply.id);
  if (!pending) return protocolFailure(state, 'candidate journal helper replied out of sequence');
  state.pending.delete(reply.id);
  clearTimeout(pending.timer);
  for (const event of reply.events) {
    if (!state.events.includes(event) && state.events.length < MAX_EVENTS) state.events.push(event);
  }
  if (reply.overflow) state.error ??= new Error('candidate journal inotify queue overflowed');
  if (!reply.ok || reply.error !== null) {
    state.error ??= new Error(reply.error || 'candidate journal helper failed');
  }
  if (state.error) pending.reject(state.error);
  else pending.resolve();
}

function consumeStdout(state, chunk) {
  if (state.closed) return;
  state.outputBytes += chunk.length;
  if (state.outputBytes > MAX_OUTPUT_BYTES) {
    return protocolFailure(state, 'candidate journal helper output exceeded limit');
  }
  state.stdout = Buffer.concat([state.stdout, chunk]);
  if (state.stdout.length > MAX_LINE_BYTES && !state.stdout.includes(0x0a)) {
    return protocolFailure(state, 'candidate journal helper protocol line exceeded limit');
  }
  let newline;
  while ((newline = state.stdout.indexOf(0x0a)) !== -1) {
    const line = state.stdout.subarray(0, newline);
    state.stdout = state.stdout.subarray(newline + 1);
    if (line.length === 0 || line.length > MAX_LINE_BYTES) {
      return protocolFailure(state, 'candidate journal helper emitted invalid framing');
    }
    const text = line.toString('utf8');
    if (!Buffer.from(text).equals(line)) {
      return protocolFailure(state, 'candidate journal helper output is not UTF-8');
    }
    try { acceptReply(state, validateReply(text)); }
    catch (error) { return protocolFailure(state, error.message); }
  }
}

function attachHelper(state) {
  state.child.stdin.on('error', (error) => {
    if (!state.closed) protocolFailure(state, error);
  });
  state.child.stdout.on('data', (chunk) => consumeStdout(state, chunk));
  state.child.stderr.on('data', (chunk) => {
    if (state.closed) return;
    state.stderrBytes += chunk.length;
    const suffix = state.stderrBytes > MAX_STDERR_BYTES ? ' exceeded limit' : '';
    protocolFailure(state, `candidate journal helper wrote stderr${suffix}`);
  });
  state.child.once('error', (error) => {
    if (!state.closed) markFailure(state, error);
  });
  state.child.once('close', (code, signal) => {
    state.helperClosed = true;
    if (!state.closed) {
      markFailure(state, new Error(
        `candidate journal helper exited unexpectedly (${signal ?? `exit ${code}`})`,
      ));
    }
  });
}

function helperState(child) {
  const state = {
    child, closed: false, closePromise: null, error: null, events: [], helperClosed: false,
    nextId: 1, outputBytes: 0, pending: new Map(), stderrBytes: 0,
    stdout: Buffer.alloc(0),
  };
  attachHelper(state);
  return state;
}

function spawnHelper(root, readyDescriptor) {
  const interpreter = trustedPythonDescriptor();
  try {
    const source = immutableHelperSource();
    const child = spawn('/proc/self/fd/4', [
      '-I', '-B', '-c', source, root, JSON.stringify(GENERATED_DIRS),
      JSON.stringify(GENERATED_PREFIXES),
    ], {
      cwd: root,
      env: { LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', PATH: '/usr/bin:/bin' },
      stdio: ['pipe', 'pipe', 'pipe', readyDescriptor, interpreter],
    });
    return helperState(child);
  } finally { closeSync(interpreter); }
}

function barrier(state) {
  if (state.closed) return Promise.reject(new Error('candidate journal is closed'));
  if (state.error) return Promise.reject(state.error);
  if (state.pending.size >= MAX_PENDING) {
    return Promise.reject(new Error('candidate journal has too many pending barriers'));
  }
  const id = state.nextId++;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      protocolFailure(state, `candidate journal barrier timed out after ${BARRIER_TIMEOUT_MS}ms`);
    }, BARRIER_TIMEOUT_MS);
    state.pending.set(id, { reject, resolve, timer });
    state.child.stdin.write(`${JSON.stringify({ id, op: 'BARRIER' })}\n`, (error) => {
      if (error && state.pending.has(id)) protocolFailure(state, error);
    });
  });
}

function closeJournal(state) {
  if (state.closePromise) return state.closePromise;
  state.closed = true;
  rejectPending(state, new Error('candidate journal is closed'));
  state.closePromise = new Promise((resolve, reject) => {
    if (state.helperClosed) {
      resolve();
      return;
    }
    let killTimer;
    const timeout = setTimeout(() => {
      stopHelper(state);
      reject(new Error(`candidate journal helper did not close after ${CLOSE_TIMEOUT_MS}ms`));
    }, CLOSE_TIMEOUT_MS);
    const finish = () => {
      clearTimeout(killTimer);
      clearTimeout(timeout);
      resolve();
    };
    state.child.once('close', finish);
    try {
      state.child.stdin.end(`${JSON.stringify({ id: state.nextId++, op: 'CLOSE' })}\n`);
    } catch {
      stopHelper(state);
    }
    killTimer = setTimeout(() => stopHelper(state), CLOSE_GRACE_MS);
  });
  return state.closePromise;
}

export function createCandidateJournal(root) {
  const absolute = realpathSync(root);
  validateHost(absolute);
  const directory = openControlDirectory(absolute);
  let ready;
  let state;
  try {
    ready = openReadyFile(directory);
    state = spawnHelper(absolute, ready.descriptor);
    waitForReady(ready);
    validateHost(absolute);
  } catch (error) {
    if (state) { state.closed = true; stopHelper(state); }
    throw error;
  } finally { cleanupReady(directory, ready); }
  return {
    barrier: () => barrier(state),
    close: () => closeJournal(state),
    drift: () => ({ error: state.error, events: [...state.events] }),
  };
}
