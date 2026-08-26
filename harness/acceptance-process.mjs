// Process-tree tracking and fail-closed cleanup for acceptance workers.
// Kept separate from scheduling so the coordinator remains below the file-size
// gate and worker teardown cannot accidentally depend on cache policy.
import { spawn } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const PROCESS_SETTLE_ATTEMPTS = 20;
const PROCESS_SETTLE_MS = 10;
const GUARDIAN_ARG = '--worker-guardian-v1';
const GUARDIAN_EMPTY_PASSES = 3;
const GUARDIAN_INIT_TIMEOUT_MS = 5_000;
const GUARDIAN_SETTLE_ATTEMPTS = 100;
const PROCESS_WAIT = new Int32Array(new SharedArrayBuffer(4));
export const WORKER_TRACKER = Symbol('forgeAcceptanceWorkerTracker');

function processRecordsFromProc() {
  if (process.platform !== 'linux') return null;
  try {
    return readdirSync('/proc').filter((name) => /^\d+$/.test(name)).flatMap((name) => {
      try {
        const stat = readFileSync(`/proc/${name}/stat`, 'utf8');
        const fields = stat.slice(stat.lastIndexOf(') ') + 2).split(' ');
        return [{
          pid: Number(name), parent: Number(fields[1]), group: Number(fields[2]),
          identity: fields[19], state: fields[0],
        }];
      } catch { return []; }
    });
  } catch { return null; }
}

function processRecords() {
  return processRecordsFromProc() ?? [];
}

function descendantRecords(rootPid, records = processRecords()) {
  const children = new Map();
  for (const record of records) {
    if (!children.has(record.parent)) children.set(record.parent, []);
    children.get(record.parent).push(record);
  }
  const found = [];
  const visit = (parent) => {
    for (const record of children.get(parent) ?? []) {
      visit(record.pid);
      found.push(record);
    }
  };
  visit(rootPid);
  return found;
}

function signalPid(pid, signal) {
  try { process.kill(pid, signal); }
  catch (error) { if (error?.code !== 'ESRCH') throw error; }
}

function matchingProcessRecords(records) {
  const current = new Map(processRecords().map((record) => [record.pid, record]));
  return records.filter((record) => {
    const found = current.get(record.pid);
    return found?.identity === record.identity && found.state !== 'Z';
  });
}

function currentLauncher(records, launcher) {
  return records.find((record) => (
    record.pid === launcher?.pid && record.identity === launcher.identity
    && record.group === launcher.group && record.state !== 'Z'
  ));
}

export function launcherGroupRecords(records, launcher) {
  if (!currentLauncher(records, launcher)) return [];
  return records.filter((record) => record.group === launcher.group && record.state !== 'Z');
}

function taggedProcessRecords(token) {
  if (!token || process.platform !== 'linux') return [];
  const field = `FORGE_ACCEPT_WORKER_TOKEN=${token}`;
  return processRecords().filter((record) => {
    try {
      return readFileSync(`/proc/${record.pid}/environ`, 'utf8').split('\0').includes(field);
    } catch { return false; }
  });
}

function settleTaggedProcesses(token) {
  let emptyPasses = 0;
  for (let attempt = 0; attempt < GUARDIAN_SETTLE_ATTEMPTS; attempt += 1) {
    const records = taggedProcessRecords(token).filter((record) => record.pid !== process.pid);
    if (records.length === 0) emptyPasses += 1;
    else {
      emptyPasses = 0;
      signalProcessRecords(records);
    }
    if (emptyPasses >= GUARDIAN_EMPTY_PASSES) return;
    Atomics.wait(PROCESS_WAIT, 0, 0, PROCESS_SETTLE_MS);
  }
  const remaining = taggedProcessRecords(token).filter((record) => record.pid !== process.pid);
  if (remaining.length > 0) {
    throw new Error(`tagged worker processes survived SIGKILL: ${remaining.map((item) => item.pid).join(',')}`);
  }
}

function guardianEnvironment(environment) {
  const clean = { ...environment };
  delete clean.FORGE_ACCEPT_WORKER_TOKEN;
  return clean;
}

function validGuardianToken(token) {
  return typeof token === 'string'
    && /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(token);
}

function assertGuardianProcSupport() {
  if (process.platform !== 'linux') throw new Error('guardian requires Linux /proc');
  readFileSync('/proc/self/stat', 'utf8');
  readFileSync('/proc/self/environ');
  readdirSync('/proc');
}

function exitGuardianAfterCleanup(token) {
  try { settleTaggedProcesses(token); process.exit(0); }
  catch { process.exit(1); }
}

function runGuardian() {
  let token = '';
  let released = false;
  const timer = setTimeout(() => process.exit(2), GUARDIAN_INIT_TIMEOUT_MS);
  process.once('disconnect', () => {
    clearTimeout(timer);
    if (released || !token) process.exit(0);
    exitGuardianAfterCleanup(token);
  });
  process.on('message', (message) => {
    if (!token && message?.type === 'init' && validGuardianToken(message.token)) {
      try { assertGuardianProcSupport(); } catch { process.exit(2); }
      token = message.token;
      clearTimeout(timer);
      process.send?.({ type: 'ready' });
    } else if (token && message?.type === 'release' && message.token === token) {
      released = true;
      process.exit(0);
    }
  });
}

function stopGuardianChild(child) {
  try { child.disconnect(); } catch { /* channel already closed */ }
  try { child.kill('SIGKILL'); } catch { /* process already exited */ }
}

export function createWorkerGuardian(token, options = {}) {
  if (process.platform !== 'linux') {
    // Local tree cleanup still applies; /proc-backed outer-SIGKILL recovery does not.
    return Promise.resolve({ child: null, closing: true, supported: false, token });
  }
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [fileURLToPath(import.meta.url), GUARDIAN_ARG], {
      detached: true, env: guardianEnvironment(options.env ?? process.env),
      stdio: ['ignore', 'ignore', 'ignore', 'ipc'],
    });
    try { options.onSpawn?.(child.pid); } catch { /* observation cannot change supervision */ }
    child.unref();
    child.channel?.unref();
    let ready = false;
    const timer = setTimeout(() => fail(new Error('guardian readiness timed out')), GUARDIAN_INIT_TIMEOUT_MS);
    const fail = (error) => {
      if (ready) return;
      ready = true;
      clearTimeout(timer);
      stopGuardianChild(child);
      reject(error);
    };
    child.once('error', fail);
    child.once('exit', (code, signal) => fail(new Error(
      `guardian exited before readiness (${signal ? `signal ${signal}` : `exit ${code}`})`,
    )));
    child.on('message', (message) => {
      if (ready || message?.type !== 'ready') return;
      ready = true;
      clearTimeout(timer);
      resolve({ child, closing: false, supported: true, token });
    });
    child.send({ type: 'init', token }, (error) => { if (error) fail(error); });
  });
}

export function releaseWorkerGuardian(guardian) {
  if (!guardian?.supported || guardian.closing) return;
  guardian.closing = true;
  try { guardian.child.send({ type: 'release', token: guardian.token }, () => {}); }
  catch { try { guardian.child.disconnect(); } catch { /* already closed */ } }
}

export function abandonWorkerGuardian(guardian) {
  if (!guardian?.supported || guardian.closing) return;
  guardian.closing = true;
  try { guardian.child.disconnect(); } catch { /* already closed */ }
}

function signalProcessRecords(records) {
  for (const record of matchingProcessRecords(records)) signalPid(record.pid, 'SIGKILL');
}

function settleProcessRecords(select, label) {
  let remaining = select();
  for (let attempt = 0; remaining.length > 0 && attempt < PROCESS_SETTLE_ATTEMPTS; attempt += 1) {
    signalProcessRecords(remaining);
    Atomics.wait(PROCESS_WAIT, 0, 0, PROCESS_SETTLE_MS);
    remaining = select();
  }
  if (remaining.length > 0) {
    throw new Error(`${label} survived SIGKILL: ${remaining.map((record) => record.pid).join(',')}`);
  }
}

export function createWorkerTracker(child, token = '') {
  if (process.platform !== 'linux') {
    throw new Error('worker identity tracking requires Linux /proc start times');
  }
  const records = new Map();
  const first = processRecords();
  const launcher = first.find((record) => record.pid === child.pid);
  if (!launcher) throw new Error('worker launcher identity is unavailable');
  const sample = () => {
    const snapshot = processRecords();
    const root = currentLauncher(snapshot, launcher);
    if (!root) return;
    records.set(`${root.pid}:${root.identity}`, root);
    for (const record of descendantRecords(launcher.pid, snapshot)) {
      records.set(`${record.pid}:${record.identity}`, record);
    }
  };
  records.set(`${launcher.pid}:${launcher.identity}`, launcher);
  for (const record of descendantRecords(launcher.pid, first)) {
    records.set(`${record.pid}:${record.identity}`, record);
  }
  const timer = setInterval(sample, 1_000);
  timer.unref?.();
  return { launcher, records, sample, timer, token };
}

function cleanupTrackedProcesses(child) {
  if (process.platform !== 'linux') return;
  const tracker = child[WORKER_TRACKER];
  if (!tracker) return;
  clearInterval(tracker.timer);
  tracker.sample();
  const records = [...tracker.records.values()];
  const select = () => {
    const selected = [...matchingProcessRecords(records), ...taggedProcessRecords(tracker.token)];
    return [...new Map(selected.map((record) => [
      `${record.pid}:${record.identity}`, record,
    ])).values()];
  };
  settleProcessRecords(select, 'tracked worker processes');
}

function cleanupPrivateProcessGroup(child) {
  if (process.platform !== 'linux') {
    throw new Error('worker process-group cleanup requires Linux identity tracking');
  }
  const launcher = child[WORKER_TRACKER]?.launcher;
  if (!launcher) throw new Error('private worker process-group identity is unavailable');
  const select = () => launcherGroupRecords(processRecords(), launcher);
  settleProcessRecords(select, 'private worker process-group members');
}

export function cleanupWorkerProcesses(child) {
  let failure = null;
  try { cleanupPrivateProcessGroup(child); }
  catch (error) { failure = error; }
  try { cleanupTrackedProcesses(child); }
  catch (error) { failure ??= error; }
  if (failure) throw failure;
}

export function signalWorker(child) {
  if (!child?.pid) return;
  try {
    if (process.platform !== 'linux') {
      throw new Error('worker signaling requires Linux identity tracking');
    }
    child[WORKER_TRACKER]?.sample();
    const snapshot = processRecords();
    const tracker = child[WORKER_TRACKER];
    const records = [
      ...launcherGroupRecords(snapshot, tracker?.launcher),
      ...(tracker?.records.values() ?? []),
      ...taggedProcessRecords(tracker?.token),
    ];
    signalProcessRecords(records);
  } catch (error) {
    if (error?.code !== 'ESRCH') throw error;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url) && process.argv[2] === GUARDIAN_ARG) {
  runGuardian();
}
