// Linux containment regressions for acceptance worker teardown. Markers test
// post-return effects because PIDs written inside a PID namespace are not host PIDs.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import {
  chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import { PASS } from './acceptance-kernel.mjs';
import {
  runSubtasks, runWorker, trustedExecutableOwner,
} from './acceptance-parallel.mjs';
import { launcherGroupRecords } from './acceptance-process.mjs';

const DAEMON_SOURCE = String.raw`
const { existsSync, writeFileSync } = require('node:fs');
writeFileSync(process.env.FORGE_TEST_READY_FILE, 'ready');
const timer = setInterval(() => {
  if (!existsSync(process.env.FORGE_TEST_TRIGGER_FILE)) return;
  writeFileSync(process.env.FORGE_TEST_ESCAPE_FILE, 'escaped');
  clearInterval(timer);
}, 10);
`;
const LAUNCHER_SOURCE = String.raw`
const { spawn } = require('node:child_process');
const child = spawn(process.execPath, ['-e', process.env.FORGE_TEST_DAEMON_SOURCE], {
  detached: true, stdio: 'ignore', env: { ...process.env },
});
child.unref();
`;
const PRIVATE_MOUNT_ARGS = [
  '--user', '--map-root-user', '--mount', '--fork', '--propagation=private', '--',
];
const MOUNT_UNAVAILABLE = /operation not permitted|permission denied|must be superuser|user namespace/i;
// Formal acceptance already reaches this suite through one contained worker.
// Only direct initial-namespace runs can safely exercise a second launcher.
const INITIAL_USER_NAMESPACE = process.platform === 'linux'
  && trustedExecutableOwner(0, readFileSync('/proc/self/uid_map', 'utf8'));

function workerTestName(name) {
  return INITIAL_USER_NAMESPACE ? name : `${name} (non-initial fail-closed alternate)`;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function makeRoot() {
  const root = mkdtempSync(join(tmpdir(), 'accept-process-test-'));
  mkdirSync(join(root, '.agent'));
  writeFileSync(join(root, '.agent', 'project.yml'), 'lifecycle: mvp\n');
  return root;
}

function controls(root) {
  const ready = join(root, 'daemon.ready');
  const trigger = join(root, 'daemon.trigger');
  const escaped = join(root, 'daemon.escaped');
  return {
    ready, trigger, escaped,
    env: {
      FORGE_TEST_READY_FILE: ready,
      FORGE_TEST_TRIGGER_FILE: trigger,
      FORGE_TEST_ESCAPE_FILE: escaped,
    },
  };
}

function fixtureWorker(root) {
  const path = join(root, 'fixture-worker.mjs');
  writeFileSync(path, `
import { spawn } from 'node:child_process';
import { existsSync, writeFileSync } from 'node:fs';
const name = process.argv[2];
const daemonSource = ${JSON.stringify(DAEMON_SOURCE)};
const launcherSource = ${JSON.stringify(LAUNCHER_SOURCE)};
const verdict = (detail = 'ok') => JSON.stringify({criterion:name,status:'PASS',detail});
function launchDaemon() {
  const env = { ...process.env, FORGE_TEST_DAEMON_SOURCE: daemonSource };
  delete env.FORGE_ACCEPT_WORKER_TOKEN;
  const child = spawn(process.execPath, ['-e', launcherSource], {
    detached: true, stdio: 'ignore', env,
  });
  child.unref();
  const wait = new Int32Array(new SharedArrayBuffer(4));
  for (let attempt = 0; attempt < 2000 && !existsSync(env.FORGE_TEST_READY_FILE); attempt += 1) {
    Atomics.wait(wait, 0, 0, 1);
  }
  if (!existsSync(env.FORGE_TEST_READY_FILE)) throw new Error('daemon did not become ready');
}
if (name === 'timeout' || name === 'build' || name === 'outer-daemon') {
  launchDaemon();
  setInterval(() => {}, 1000);
}
if (name === 'pass-daemon') {
  launchDaemon();
  process.stdout.write(verdict());
}
if (name === 'escape') {
  process.stdout.write(verdict(), () => launchDaemon());
}
if (name === 'hold-pass') {
  writeFileSync(process.env.FORGE_TEST_READY_FILE, 'ready');
  setTimeout(() => process.stdout.write(verdict()), 250);
}
if (name === 'inspect') {
  process.stdout.write(verdict(String(process.ppid) + ':'
    + String('FORGE_ACCEPT_WORKER_TOKEN' in process.env)));
}
`);
  return path;
}

function reaperFixtureWorker(root) {
  const path = join(root, 'reaper-worker.mjs');
  writeFileSync(path, `
import { spawn } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
const child = spawn('/bin/sh', ['-c', 'sleep 0.01 &'], { detached:true, stdio:'ignore' });
child.unref();
setTimeout(() => {
  const zombies = readdirSync('/proc').filter((name) => /^\\d+$/.test(name)).filter((name) => {
    try { return readFileSync('/proc/' + name + '/stat', 'utf8').split(') ')[1][0] === 'Z'; }
    catch { return false; }
  });
  process.stdout.write(JSON.stringify({criterion:'reap',status:'PASS',detail:String(zombies.length)}));
}, 250);
`);
  return path;
}

function mountProbeScript(root) {
  const path = join(root, 'mount-probe.mjs');
  const moduleUrl = new URL('./acceptance-parallel.mjs', import.meta.url).href;
  writeFileSync(path, `
import { spawnSync } from 'node:child_process';
import { existsSync, realpathSync, writeFileSync } from 'node:fs';
import { runWorker } from ${JSON.stringify(moduleUrl)};
const [mode, root, worker, attacker, marker] = process.argv.slice(2);
const bind = (source, target) => spawnSync(
  '/usr/bin/mount', ['--bind', source, target], { encoding: 'utf8' },
);
const stopUnavailable = (runs) => {
  const failed = runs.find((run) => run?.status !== 0);
  if (!failed) return;
  const reason = failed.stderr || failed.error?.message || 'mount failed';
  writeFileSync(1, JSON.stringify({ skip: reason }));
  process.exit(77);
};
let mounts = [];
if (mode === 'pre') {
  mounts = [
    bind(attacker, '/usr/bin/unshare'),
    bind(process.execPath, '/usr/bin/env'),
    bind(process.execPath, realpathSync('/bin/sh')),
  ];
  stopUnavailable(mounts);
}
const options = {
  root, workerPath: worker, timeoutMs: 2_000,
  env: { ...process.env, FORGE_MOUNT_ATTACK_MARKER: marker },
};
if (mode === 'post') options.onGuardianSpawn = () => {
  mounts = [bind(attacker, '/usr/bin/unshare')];
};
const row = await runWorker('inspect', options);
if (mode === 'post') stopUnavailable(mounts);
writeFileSync(1, JSON.stringify({ row, attacked: existsSync(marker) }));
`);
  return path;
}

function runPrivateMountProbe(t, mode, nested) {
  const root = makeRoot();
  const attacker = join(root, 'attacker.py');
  const marker = join(root, 'attacker-ran');
  try {
    writeFileSync(attacker, '#!/usr/bin/python3\nimport os\nopen(os.environ["FORGE_MOUNT_ATTACK_MARKER"], "w").write("ran")\n');
    chmodSync(attacker, 0o755);
    const command = [
      ...PRIVATE_MOUNT_ARGS,
      ...(nested ? ['/usr/bin/unshare', ...PRIVATE_MOUNT_ARGS] : []),
      process.execPath, mountProbeScript(root), mode, root,
      fixtureWorker(root), attacker, marker,
    ];
    const run = spawnSync('/usr/bin/unshare', command, {
      encoding: 'utf8', timeout: 15_000,
    });
    const combined = `${run.stdout || ''}\n${run.stderr || ''}`;
    if (run.status === 77 || MOUNT_UNAVAILABLE.test(combined)) {
      t.skip(`private rootless mount unavailable: ${combined.trim()}`);
      return null;
    }
    assert.equal(run.status, 0, `private mount probe failed: ${combined}`);
    return JSON.parse(run.stdout);
  } finally { rmSync(root, { recursive: true, force: true }); }
}

function processAlive(pid) {
  try { process.kill(pid, 0); return true; }
  catch (error) { return error?.code !== 'ESRCH'; }
}

async function waitForDead(pid) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (!processAlive(pid)) return true;
    await delay(20);
  }
  return false;
}

async function waitForFile(path) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (existsSync(path)) return true;
    await delay(20);
  }
  return false;
}

async function assertContained(control) {
  assert.equal(readFileSync(control.ready, 'utf8'), 'ready');
  writeFileSync(control.trigger, 'release');
  await delay(400);
  assert.equal(existsSync(control.escaped), false, 'daemon performed an effect after worker return');
}

async function cleanupRoot(root, control) {
  try { writeFileSync(control.trigger, 'release'); } catch { /* root may not exist */ }
  await delay(100);
  rmSync(root, { recursive: true, force: true });
}

async function nestedWorkerRejected() {
  if (INITIAL_USER_NAMESPACE) return false;
  const root = makeRoot();
  try {
    const row = await runWorker('inspect', { root, workerPath: fixtureWorker(root) });
    assert.equal(row.status, 'FAIL');
    assert.match(row.detail, /worker containment failed/);
  } finally { rmSync(root, { recursive: true, force: true }); }
  return true;
}

test('private group selection is bound to the original launcher start identity', () => {
  const launcher = { pid: 40, parent: 1, group: 40, identity: 'start-a', state: 'S' };
  const member = { pid: 41, parent: 40, group: 40, identity: 'start-b', state: 'S' };
  assert.deepEqual(launcherGroupRecords([launcher, member], launcher), [launcher, member]);
  const reused = { ...launcher, identity: 'start-reused' };
  const unrelated = { ...member, parent: 40, identity: 'start-unrelated' };
  assert.deepEqual(launcherGroupRecords([reused, unrelated], launcher), []);
});

test('trusted executable ownership requires the initial user namespace', () => {
  const initial = '         0          0 4294967295\n';
  const rootless = '      1000       1000          1\n';
  assert.equal(trustedExecutableOwner(0, initial), true);
  assert.equal(trustedExecutableOwner(1000, initial), false);
  assert.equal(trustedExecutableOwner(65534, rootless), false);
  assert.equal(trustedExecutableOwner(1000, rootless), false);
  assert.equal(trustedExecutableOwner(0, rootless), false);
  assert.equal(trustedExecutableOwner(0, '0 0 1\n'), false);
  assert.equal(trustedExecutableOwner(65534, '0 0 1\n'), false);
  assert.equal(trustedExecutableOwner(0, 'malformed'), false);
});

test('nested rootless mount replacement cannot impersonate host executables', {
  skip: process.platform !== 'linux',
}, (t) => {
  const result = runPrivateMountProbe(t, 'pre', true);
  if (!result) return;
  assert.equal(result.row.status, 'FAIL');
  assert.match(result.row.detail, /containment executable is unsafe/);
  assert.equal(result.attacked, false);
});

test('non-initial namespace fails before a post-validation mount replacement', {
  skip: process.platform !== 'linux',
}, (t) => {
  const result = runPrivateMountProbe(t, 'post', false);
  if (!result) return;
  assert.equal(result.row.status, 'FAIL');
  assert.match(result.row.detail, /containment executable is unsafe/);
  assert.equal(result.attacked, false);
});

test(workerTestName('Linux namespace PID 1 reaps adopted short-lived grandchildren'), async () => {
  if (await nestedWorkerRejected()) return;
  const root = makeRoot();
  try {
    const result = await runWorker('reap', { root, workerPath: reaperFixtureWorker(root) });
    assert.equal(result.status, PASS);
    assert.equal(result.detail, '0');
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test(workerTestName('Linux namespace hides the guardian token and ignores a PATH-injected unshare'), async () => {
  if (await nestedWorkerRejected()) return;
  const root = makeRoot();
  const marker = join(root, 'fake-unshare-ran');
  const fake = join(root, 'unshare');
  try {
    writeFileSync(fake, `#!/bin/sh\nprintf ran > ${JSON.stringify(marker)}\n`);
    chmodSync(fake, 0o755);
    const result = await runWorker('inspect', {
      root, workerPath: fixtureWorker(root), env: { PATH: root },
    });
    assert.equal(result.status, PASS);
    assert.equal(result.detail, '1:false');
    assert.equal(existsSync(marker), false);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test(workerTestName('timeout kills a tokenless detached double-fork before returning'), async () => {
  if (await nestedWorkerRejected()) return;
  const root = makeRoot(); const control = controls(root); let guardianPid;
  try {
    const result = await runWorker('timeout', {
      root, workerPath: fixtureWorker(root), timeoutMs: 100, env: control.env,
      onGuardianSpawn: (pid) => { guardianPid = pid; },
    });
    assert.match(result.detail, /timed out after 100ms/);
    await assertContained(control);
    assert.equal(await waitForDead(guardianPid), true);
  } finally { await cleanupRoot(root, control); }
});

test(workerTestName('total scheduler timeout kills its namespace before resolving red'), async () => {
  if (await nestedWorkerRejected()) return;
  const root = makeRoot(); const control = controls(root);
  const resultFile = join(root, 'result.json');
  const supervisorPath = join(root, 'supervisor.mjs');
  const moduleUrl = new URL('./acceptance-parallel.mjs', import.meta.url).href;
  writeFileSync(supervisorPath, `
import { writeFileSync } from 'node:fs';
import { runSubtasks } from ${JSON.stringify(moduleUrl)};
const started = Date.now();
const rows = await runSubtasks(['build'], {
  root: ${JSON.stringify(root)}, workerPath: ${JSON.stringify(fixtureWorker(root))},
  env: ${JSON.stringify(control.env)}, timeoutMs: 5000, totalTimeoutMs: 150,
});
writeFileSync(${JSON.stringify(resultFile)}, JSON.stringify({rows, elapsed:Date.now()-started}));
`);
  const supervisor = spawn(process.execPath, [supervisorPath], { detached: true, stdio: 'ignore' });
  const closed = once(supervisor, 'close');
  try {
    await closed;
    const result = JSON.parse(readFileSync(resultFile, 'utf8'));
    assert.equal(result.rows[0].status, 'FAIL');
    assert.ok(result.elapsed < 3000, `total timeout settled after ${result.elapsed}ms`);
    await assertContained(control);
  } finally { await cleanupRoot(root, control); }
});

test(workerTestName('PASS cannot leave a tokenless post-output double-fork alive'), async () => {
  if (await nestedWorkerRejected()) return;
  const root = makeRoot(); const control = controls(root); let guardianPid;
  try {
    const result = await runWorker('escape', {
      root, workerPath: fixtureWorker(root), env: control.env,
      onGuardianSpawn: (pid) => { guardianPid = pid; },
    });
    assert.equal(result.status, PASS);
    await assertContained(control);
    assert.equal(await waitForDead(guardianPid), true);
  } finally { await cleanupRoot(root, control); }
});

test(workerTestName('private worker cleanup never kills a new coordinator-group process'), async () => {
  if (await nestedWorkerRejected()) return;
  const root = makeRoot(); const control = controls(root); let sentinel;
  try {
    const pending = runWorker('hold-pass', {
      root, workerPath: fixtureWorker(root), env: control.env, exclusiveGroupCleanup: true,
    });
    assert.equal(await waitForFile(control.ready), true);
    sentinel = spawn(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], { stdio: 'ignore' });
    const result = await pending;
    assert.equal(result.status, PASS);
    assert.equal(processAlive(sentinel.pid), true, 'unrelated coordinator-group process was killed');
  } finally {
    if (sentinel && processAlive(sentinel.pid)) sentinel.kill('SIGKILL');
    await cleanupRoot(root, control);
  }
});

test(workerTestName('guardian destroys the namespace after outer coordinator SIGKILL'), async () => {
  if (await nestedWorkerRejected()) return;
  const root = makeRoot(); const control = controls(root);
  const guardianFile = join(root, 'guardian.pid');
  const supervisorPath = join(root, 'outer-supervisor.mjs');
  const moduleUrl = new URL('./acceptance-parallel.mjs', import.meta.url).href;
  writeFileSync(supervisorPath, `
import { writeFileSync } from 'node:fs';
import { runWorker } from ${JSON.stringify(moduleUrl)};
await runWorker('outer-daemon', {
  root: ${JSON.stringify(root)}, workerPath: ${JSON.stringify(fixtureWorker(root))},
  env: ${JSON.stringify(control.env)},
  onGuardianSpawn: (pid) => writeFileSync(${JSON.stringify(guardianFile)}, String(pid)),
});
`);
  const supervisor = spawn(process.execPath, [supervisorPath], { detached: true, stdio: 'ignore' });
  const closed = once(supervisor, 'close'); let guardianPid;
  try {
    assert.equal(await waitForFile(control.ready), true);
    guardianPid = Number(readFileSync(guardianFile, 'utf8'));
    assert.doesNotMatch(readFileSync(`/proc/${guardianPid}/environ`, 'utf8'),
      /FORGE_ACCEPT_WORKER_TOKEN=/);
    process.kill(-supervisor.pid, 'SIGKILL');
    await closed;
    assert.equal(await waitForDead(guardianPid), true);
    await assertContained(control);
  } finally {
    if (processAlive(supervisor.pid)) process.kill(-supervisor.pid, 'SIGKILL');
    if (guardianPid && processAlive(guardianPid)) process.kill(guardianPid, 'SIGKILL');
    await cleanupRoot(root, control);
  }
});
