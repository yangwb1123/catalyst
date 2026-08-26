// Bounded process-parallel acceptance coordinator. Node is only the supervisor:
// each probe runs in an independent process, so Node/Python/Go/Rust workloads
// execute on multiple cores. This module is live-only and has no cache import;
// the explicit --cache path is isolated in acceptance-advisory.mjs.
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import {
  closeSync, constants as fsConstants, fstatSync, lstatSync, openSync,
  readFileSync, realpathSync,
} from 'node:fs';
import { availableParallelism, cpus } from 'node:os';
import { join } from 'node:path';

import {
  APPLICABLE, FAIL, HARNESS_DIR, INAPPLICABLE, NA, PASS, ROOT, withCategory,
} from './acceptance-kernel.mjs';
import {
  candidateFingerprint, candidateInventory, candidateInventoryDiff,
} from './acceptance-candidate.mjs';
import {
  abandonWorkerGuardian, cleanupWorkerProcesses, createWorkerGuardian,
  createWorkerTracker, releaseWorkerGuardian, signalWorker, WORKER_TRACKER,
} from './acceptance-process.mjs';
import { ACCEPTANCE_CRITERIA, aggregateTestPass } from './acceptance.mjs';

const WORKER = join(HARNESS_DIR, 'acceptance-worker.mjs');
const DEFAULT_OUTPUT_MAX_BYTES = 1024 * 1024;
const DEFAULT_WORKER_TIMEOUT_MS = 4 * 60 * 60 * 1000;
const DEFAULT_TOTAL_TIMEOUT_MS = 4 * 60 * 60 * 1000;
const HARD_SETTLE_MS = 2_000;
// Each probe fans out internally (node --test, go test, Cargo, coverage). Four
// top-level probes preserve overlap without multiplying that nested fan-out by
// every available CPU on large hosts. Operators may tune the explicit env knob.
const DEFAULT_MAX_CONCURRENT_PROBES = 4;
const VALID_STATUSES = new Set([PASS, FAIL, NA]);
const VALID_CATEGORIES = new Set([APPLICABLE, INAPPLICABLE, 'no_tool']);
const LINUX_UNSHARE = '/usr/bin/unshare';
const LINUX_ENV = '/usr/bin/env';
const LINUX_SHELL = '/bin/sh';
const LINUX_REAPER_SCRIPT = '"$@" & worker=$!; wait "$worker"';
const LINUX_NAMESPACE_ARGS = [
  '--user', '--map-current-user', '--pid', '--fork',
  '--kill-child=SIGKILL', '--mount-proc', '--propagation=private', '--',
];

export const READ_ONLY_SUBTASKS = [
  'complexity_violations', 'arch_violations', 'architecture',
  'security_findings', 'dependency_vulnerabilities',
];
// The two harness test families use separate runtimes and do not share output
// artifacts, so they may overlap in one bounded stage.
export const PARALLEL_SUBTASKS = [
  'test_pass_node', 'test_pass_python',
];
// Arbitrary project commands can share generated targets, caches and artifact
// paths. They run as exclusive stages so one probe cannot race another over a
// Maven/Gradle/Cargo/npm/coverage output, and group cleanup cannot kill a live
// sibling worker.
export const SERIAL_SUBTASKS = [
  'test_pass_project', 'app_test_pass', 'lint', 'coverage', 'typecheck', 'build',
];
export const SUBTASK_STAGES = [
  READ_ONLY_SUBTASKS, PARALLEL_SUBTASKS,
  ...SERIAL_SUBTASKS.map((name) => [name]),
];
export const SUBTASKS = SUBTASK_STAGES.flat();
export const parallelSchemaOrder = ACCEPTANCE_CRITERIA;

function failureRow(name, detail) {
  return withCategory({ criterion: name, status: FAIL, detail });
}

function categoryMatchesStatus(row) {
  if (row.category === undefined) return true;
  return row.status === NA ? row.category !== APPLICABLE : row.category === APPLICABLE;
}

function normalizeRow(name, row) {
  const fields = row && !Array.isArray(row) && typeof row === 'object'
    ? Object.keys(row) : [];
  if (!row || Array.isArray(row) || typeof row !== 'object'
      || fields.length < 3 || fields.length > 4
      || fields.some((field) => !['criterion', 'status', 'detail', 'category'].includes(field))
      || row.criterion !== name || !VALID_STATUSES.has(row.status)
      || typeof row.detail !== 'string' || Buffer.byteLength(row.detail) > DEFAULT_OUTPUT_MAX_BYTES
      || (row.category !== undefined && !VALID_CATEGORIES.has(row.category))
      || !categoryMatchesStatus(row)) {
    return failureRow(name, `${name}: worker returned an invalid verdict`);
  }
  return withCategory({
    criterion: name, status: row.status, detail: row.detail, category: row.category,
  });
}

function appendCapture(capture, stream, chunk, limit) {
  const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
  capture.total += bytes.length;
  const remaining = Math.max(0, limit - capture.retained);
  if (remaining > 0) {
    capture[stream].push(bytes.subarray(0, remaining));
    capture.retained += Math.min(bytes.length, remaining);
  }
  return capture.total > limit;
}

function capturedText(capture, stream) {
  return Buffer.concat(capture[stream]).toString('utf8');
}

function decodeWorkerRow(name, code, signal, capture) {
  const stderr = capturedText(capture, 'stderr').trim().slice(0, 1000);
  if (code !== 0) {
    const exit = signal ? `signal ${signal}` : `exit ${code ?? 'unknown'}`;
    return failureRow(name, `${name}: worker ${exit}${stderr ? `: ${stderr}` : ''}`);
  }
  try {
    const raw = capturedText(capture, 'stdout');
    const parsed = JSON.parse(raw);
    if (JSON.stringify(parsed) !== raw) throw new Error('non-canonical or duplicate-key JSON');
    return normalizeRow(name, parsed);
  } catch (error) {
    return failureRow(name, `${name}: worker emitted invalid JSON (${error.message})`);
  }
}

export function trustedExecutableOwner(uid, uidMapText) {
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
  return mappings.length === 1
    && mappings[0][0] === 0 && mappings[0][1] === 0
    && mappings[0][2] === 4_294_967_295 && uid === 0;
}

function trustedSystemExecutable(path) {
  let descriptor;
  try {
    const target = realpathSync(path);
    const before = lstatSync(target);
    descriptor = openSync(target, fsConstants.O_RDONLY | fsConstants.O_NOFOLLOW);
    const opened = fstatSync(descriptor);
    const uidMap = readFileSync('/proc/self/uid_map', 'utf8');
    const sameFile = before.dev === opened.dev && before.ino === opened.ino;
    if (!sameFile || !opened.isFile() || (opened.mode & 0o111) === 0
        || (opened.mode & 0o022) !== 0
        || !trustedExecutableOwner(opened.uid, uidMap)) {
      throw new Error('unsafe executable identity');
    }
    return descriptor;
  } catch (error) {
    if (descriptor !== undefined) closeSync(descriptor);
    throw new Error(`worker containment executable is unsafe: ${path} (${error.message})`);
  }
}

function workerCommand(workerArgs) {
  if (process.platform !== 'linux') {
    throw new Error('formal worker containment requires Linux user/PID namespaces');
  }
  const descriptors = [];
  try {
    descriptors.push(trustedSystemExecutable(LINUX_UNSHARE));
    descriptors.push(trustedSystemExecutable(LINUX_ENV));
    descriptors.push(trustedSystemExecutable(LINUX_SHELL));
    return {
      command: '/proc/self/fd/3', descriptors,
      args: [
        ...LINUX_NAMESPACE_ARGS, '/proc/self/fd/4', '-u', 'FORGE_ACCEPT_WORKER_TOKEN',
        '/proc/self/fd/5', '-c', LINUX_REAPER_SCRIPT,
        'forge-accept-init', process.execPath, ...workerArgs,
      ],
    };
  } catch (error) {
    for (const descriptor of descriptors) closeSync(descriptor);
    throw error;
  }
}

function workerOptions(name, options) {
  const token = randomUUID();
  const launch = workerCommand([options.workerPath ?? WORKER, name]);
  const stdio = launch.descriptors
    ? ['ignore', 'pipe', 'pipe', ...launch.descriptors]
    : ['ignore', 'pipe', 'pipe'];
  return {
    token, ...launch,
    spawn: {
      cwd: options.root ?? ROOT,
      // A private POSIX group makes local cleanup exact. Linux additionally
      // uses a PID namespace so setsid/double-fork cannot escape containment.
      detached: process.platform !== 'win32',
      env: {
        ...(options.env ?? process.env),
        FORGE_ACCEPT_NO_CACHE: '1', FORGE_ACCEPT_WORKER_TOKEN: token,
      },
      stdio,
    },
  };
}

function spawnWorker(config) {
  try { return spawn(config.command, config.args, config.spawn); }
  finally {
    for (const descriptor of config.descriptors ?? []) closeSync(descriptor);
    config.descriptors = [];
  }
}

function resolveWorkerCleanup(name, options, child, guardian, resolve, row) {
  try {
    cleanupWorkerProcesses(child);
    releaseWorkerGuardian(guardian);
    resolve(row);
  } catch (error) {
    abandonWorkerGuardian(guardian);
    if (options.cleanupState) options.cleanupState.failed = true;
    resolve(failureRow(name, `${name}: worker cleanup failed (${error.message})`));
  }
}

function bindWorkerCompletion(child, guardian, finish, stop, verdict) {
  guardian.child?.once('exit', () => {
    if (!guardian.closing) stop('guardian exited unexpectedly');
  });
  child.once('error', (error) => {
    finish(failureRow(verdict.name, `${verdict.name}: worker spawn failed (${error.message})`));
  });
  child.once('close', (code, signal) => {
    finish(verdict.reason()
      ? failureRow(verdict.name, `${verdict.name}: worker ${verdict.reason()}`)
      : decodeWorkerRow(verdict.name, code, signal, verdict.capture));
  });
}

function workerCapture(options) {
  return {
    capture: { stdout: [], stderr: [], retained: 0, total: 0 },
    outputLimit: options.outputMaxBytes ?? DEFAULT_OUTPUT_MAX_BYTES,
    active: options.activeChildren,
  };
}

function superviseWorker(name, options, config, guardian) {
  return new Promise((resolve) => {
    let child;
    try { child = spawnWorker(config); }
    catch (error) {
      abandonWorkerGuardian(guardian);
      resolve(failureRow(name, `${name}: worker spawn failed (${error.message})`));
      return;
    }
    try { child[WORKER_TRACKER] = createWorkerTracker(child, config.token); }
    catch (error) {
      child.once('error', () => {}); try { child.kill('SIGKILL'); } catch { /* already gone */ }
      abandonWorkerGuardian(guardian);
      resolve(failureRow(name, `${name}: worker tracking failed (${error.message})`));
      return;
    }
    const { capture, outputLimit, active } = workerCapture(options);
    let reason = null; let settled = false; let timeout = null; let hardTimer = null;
    active?.add(child);
    const finish = (row) => {
      if (settled) return;
      settled = true;
      if (timeout) clearTimeout(timeout);
      if (hardTimer) clearTimeout(hardTimer);
      active?.delete(child);
      resolveWorkerCleanup(name, options, child, guardian, resolve, row);
    };
    const stop = (why) => {
      if (reason) return;
      reason = why;
      try { signalWorker(child); } catch { /* hard-settle still returns FAIL */ }
      child.stdout.destroy();
      child.stderr.destroy();
      hardTimer = setTimeout(() => finish(failureRow(name, `${name}: worker ${why}`)), HARD_SETTLE_MS);
    };
    const captureChunk = (stream, chunk) => {
      child[WORKER_TRACKER].sample();
      if (appendCapture(capture, stream, chunk, outputLimit)) stop('output limit exceeded');
    };
    child.stdout.on('data', (chunk) => captureChunk('stdout', chunk));
    child.stderr.on('data', (chunk) => captureChunk('stderr', chunk));
    const timeoutMs = options.timeoutMs ?? DEFAULT_WORKER_TIMEOUT_MS;
    timeout = setTimeout(() => stop(`timed out after ${timeoutMs}ms`), timeoutMs);
    timeout.unref?.();
    bindWorkerCompletion(child, guardian, finish, stop, {
      capture, name, reason: () => reason,
    });
  });
}

export async function runWorker(name, options = {}) {
  let config;
  try { config = workerOptions(name, options); }
  catch (error) {
    return failureRow(name, `${name}: worker containment failed (${error.message})`);
  }
  let guardian;
  try {
    guardian = await createWorkerGuardian(config.token, {
      env: config.spawn.env, onSpawn: options.onGuardianSpawn,
    });
  } catch (error) {
    return failureRow(name, `${name}: worker guardian failed (${error.message})`);
  }
  return superviseWorker(name, options, config, guardian);
}

export function defaultConcurrency(taskCount = SUBTASKS.length) {
  const available = typeof availableParallelism === 'function'
    ? availableParallelism() : cpus().length;
  return Math.max(1, Math.min(taskCount, available || 1, DEFAULT_MAX_CONCURRENT_PROBES));
}

function integerOption(value, fallback, label, allowZero = true) {
  if (value === undefined || value === null || value === '') return fallback;
  const parsed = Number(value);
  const minimum = allowZero ? 0 : 1;
  if (!Number.isSafeInteger(parsed) || parsed < minimum) {
    throw new Error(`${label} must be ${allowZero ? 'a non-negative' : 'a positive'} integer`);
  }
  return parsed;
}

export async function runPool(names, concurrency, task) {
  const rows = new Array(names.length);
  let cursor = 0;
  async function lane() {
    while (cursor < names.length) {
      const index = cursor;
      cursor += 1;
      rows[index] = await task(names[index]);
    }
  }
  const lanes = Math.max(1, Math.min(names.length, concurrency));
  await Promise.all(Array.from({ length: lanes }, () => lane()));
  return rows;
}

export async function runStages(names, concurrency, task) {
  const requested = new Set(names);
  if (requested.size !== names.length || names.some((name) => !SUBTASKS.includes(name))) {
    throw new Error('acceptance scheduler received duplicate or unknown probe names');
  }
  const rows = [];
  for (const stage of SUBTASK_STAGES) {
    const selected = stage.filter((name) => requested.has(name));
    if (selected.length > 0) rows.push(...await runPool(selected, concurrency, task));
  }
  return rows;
}

export function assembleResults(byName) {
  const project = byName.get('test_pass_project');
  const test = aggregateTestPass(
    byName.get('test_pass_node'), byName.get('test_pass_python'), project,
  );
  const rows = new Map(byName);
  rows.set('test_pass', withCategory(test));
  rows.delete('test_pass_node');
  rows.delete('test_pass_python');
  rows.delete('test_pass_project');
  return parallelSchemaOrder.map((name) => withCategory(rows.get(name)));
}

function installSignalCleanup(active) {
  const handlers = new Map();
  for (const signal of ['SIGINT', 'SIGTERM']) {
    const handler = () => {
      for (const child of active) {
        try { signalWorker(child); } catch { /* best effort */ }
      }
      process.removeListener(signal, handler);
      process.kill(process.pid, signal);
    };
    handlers.set(signal, handler);
    process.once(signal, handler);
  }
  return () => {
    for (const [signal, handler] of handlers) process.removeListener(signal, handler);
  };
}

function installTotalTimeout(active, timeoutMs, state) {
  const timer = setTimeout(() => {
    state.expired = true;
    for (const child of active) {
      try { signalWorker(child); } catch { /* hard-settle owns final failure */ }
    }
  }, timeoutMs);
  timer.unref?.();
  return timer;
}

export async function runSubtasks(names, options = {}) {
  const root = options.root ?? ROOT;
  const env = options.env ?? process.env;
  const concurrency = integerOption(
    options.concurrency ?? env.FORGE_ACCEPT_CONCURRENCY,
    defaultConcurrency(names.length), 'FORGE_ACCEPT_CONCURRENCY', false,
  );
  const totalTimeoutMs = integerOption(
    options.totalTimeoutMs ?? env.FORGE_ACCEPT_TOTAL_TIMEOUT_MS,
    DEFAULT_TOTAL_TIMEOUT_MS, 'total acceptance timeout', false,
  );
  const workerTimeoutMs = integerOption(
    options.timeoutMs ?? env.FORGE_ACCEPT_WORKER_TIMEOUT_MS,
    DEFAULT_WORKER_TIMEOUT_MS, 'worker timeout', false,
  );
  const outputMaxBytes = integerOption(
    options.outputMaxBytes, DEFAULT_OUTPUT_MAX_BYTES, 'worker output cap', false,
  );
  const activeChildren = new Set();
  const cleanupState = { failed: false };
  const removeSignals = installSignalCleanup(activeChildren);
  const runner = options.runTask ?? runWorker;
  const deadline = Date.now() + totalTimeoutMs;
  const state = { expired: false };
  const totalTimer = installTotalTimeout(activeChildren, totalTimeoutMs, state);
  const worker = {
    root, env, activeChildren, cleanupState,
    timeoutMs: workerTimeoutMs, outputMaxBytes,
    workerPath: options.workerPath,
  };
  try {
    return await runStages(names, Math.max(1, concurrency), async (name) => {
      if (state.expired) return failureRow(name, `${name}: total acceptance timeout expired`);
      if (cleanupState.failed) return failureRow(name, `${name}: prior worker cleanup failed`);
      const scoped = {
        ...worker,
        exclusiveGroupCleanup: SERIAL_SUBTASKS.includes(name),
        timeoutMs: Math.min(worker.timeoutMs, Math.max(1, deadline - Date.now())),
      };
      try { return normalizeRow(name, await runner(name, scoped)); }
      catch (error) { return failureRow(name, `${name}: worker runner crashed (${error.message})`); }
    });
  } finally {
    clearTimeout(totalTimer);
    removeSignals();
    for (const child of activeChildren) {
      try { signalWorker(child); } catch { /* final fail-safe */ }
    }
  }
}

export async function collectParallel(options = {}) {
  if (options.useCache === true) {
    throw new Error('acceptance-parallel is live-only; use the explicit advisory coordinator');
  }
  const formal = await import('./acceptance/formal.mjs');
  return formal.collectFormal(options, {
    assembleResults, candidateFingerprint, candidateInventory, candidateInventoryDiff,
    FAIL, parallelSchemaOrder, ROOT, runSubtasks, SUBTASKS, withCategory,
  });
}
