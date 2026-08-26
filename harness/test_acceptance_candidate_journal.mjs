import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import { createCandidateJournal } from './acceptance/candidate-journal.mjs';

const PRIVATE_MOUNT_ARGS = [
  '--user', '--map-root-user', '--mount', '--fork', '--propagation=private', '--',
];
const MOUNT_UNAVAILABLE = /operation not permitted|permission denied|must be superuser|user namespace/i;

function inInitialUserNamespace() {
  if (process.platform !== 'linux') return false;
  try {
    const lines = readFileSync('/proc/self/uid_map', 'utf8').trim().split('\n');
    const fields = lines[0]?.trim().split(/\s+/).map(Number);
    return lines.length === 1 && fields?.length === 3
      && fields[0] === 0 && fields[1] === 0 && fields[2] === 4_294_967_295;
  } catch { return false; }
}

const CAN_CREATE_JOURNAL = inInitialUserNamespace();

function journalTestName(name) {
  return CAN_CREATE_JOURNAL ? name : `${name} (non-initial fail-closed alternate)`;
}

function fixture() {
  const root = mkdtempSync(join(tmpdir(), 'candidate-journal-'));
  mkdirSync(join(root, 'docs'));
  writeFileSync(join(root, 'docs', 'contract.md'), 'A\n');
  return root;
}

function nestedJournalRejected() {
  if (CAN_CREATE_JOURNAL) return false;
  const root = fixture();
  try {
    assert.throws(
      () => createCandidateJournal(root),
      /requires Linux|Python interpreter is unsafe/,
    );
  } finally { rmSync(root, { recursive: true, force: true }); }
  return true;
}

function helperPid(root) {
  const result = spawnSync('/usr/bin/pgrep', ['-f', '--', root], { encoding: 'utf8' });
  assert.equal(result.status, 0);
  const identifiers = result.stdout.trim().split('\n').filter(Boolean).map(Number);
  assert.equal(identifiers.length, 1, 'exactly one journal helper must be observable');
  assert.ok(Number.isSafeInteger(identifiers[0]) && identifiers[0] > 0);
  return identifiers[0];
}

function processAlive(pid) {
  try { process.kill(pid, 0); return true; }
  catch (error) { return error?.code !== 'ESRCH'; }
}

async function waitForDead(pid) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (!processAlive(pid)) return true;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  return false;
}

function mountProbeSource(root) {
  const path = join(root, 'mount-probe.mjs');
  const moduleUrl = new URL('./acceptance/candidate-journal.mjs', import.meta.url).href;
  writeFileSync(path, `
import { spawnSync } from 'node:child_process';
import { existsSync, realpathSync, writeFileSync } from 'node:fs';
const [mode, root, attacker, marker] = process.argv.slice(2);
const bind = () => spawnSync(
  '/usr/bin/mount', ['--bind', attacker, realpathSync('/usr/bin/python3')],
  { encoding: 'utf8' },
);
const stopUnavailable = (run) => {
  if (run.status === 0) return;
  writeFileSync(1, JSON.stringify({ skip: run.stderr || run.error?.message || 'mount failed' }));
  process.exit(77);
};
if (mode === 'pre') stopUnavailable(bind());
const { createCandidateJournal } = await import(${JSON.stringify(moduleUrl)});
let journal;
try {
  journal = createCandidateJournal(root);
  await journal.barrier();
  writeFileSync(1, JSON.stringify({
    ok: true, attacked: existsSync(marker), error: journal.drift().error?.message ?? null,
  }));
} catch (error) {
  writeFileSync(1, JSON.stringify({
    ok: false, attacked: existsSync(marker), error: error.message,
  }));
} finally { await journal?.close(); }
`);
  return path;
}

function runMountProbe(t, mode, nested) {
  const root = fixture();
  const marker = join(root, 'attacker-ran');
  const attacker = join(root, 'attacker.sh');
  try {
    writeFileSync(attacker, `#!/bin/sh\nprintf attacked > ${JSON.stringify(marker)}\nprintf '{"ok":false,"error":"attacker"}\\n' >&3\n`);
    chmodSync(attacker, 0o755);
    const command = [
      ...PRIVATE_MOUNT_ARGS,
      ...(nested ? ['/usr/bin/unshare', ...PRIVATE_MOUNT_ARGS] : []),
      process.execPath, mountProbeSource(root), mode, root, attacker, marker,
    ];
    const run = spawnSync('/usr/bin/unshare', command, {
      encoding: 'utf8', timeout: 20_000,
    });
    const combined = `${run.stdout || ''}\n${run.stderr || ''}`;
    if (run.status === 77 || MOUNT_UNAVAILABLE.test(combined)) {
      t.skip(`private rootless mount unavailable: ${combined.trim()}`);
      return null;
    }
    assert.equal(run.status, 0, `private mount probe failed: ${combined}`);
    const result = JSON.parse(run.stdout);
    assert.equal(existsSync(marker), false, 'attacker ran outside private namespace');
    return result;
  } finally { rmSync(root, { recursive: true, force: true }); }
}

test(journalTestName('candidate journal permanently records A-to-B-to-A drift'), async () => {
  if (nestedJournalRejected()) return;
  const root = fixture();
  const journal = createCandidateJournal(root);
  try {
    await journal.barrier();
    const path = join(root, 'docs', 'contract.md');
    writeFileSync(path, 'B\n');
    assert.equal(readFileSync(path, 'utf8'), 'B\n');
    writeFileSync(path, 'A\n');
    await journal.barrier();
    const observation = journal.drift();
    assert.equal(observation.error, null);
    assert.ok(observation.events.includes('docs/contract.md'));
  } finally {
    await journal.close();
    rmSync(root, { recursive: true, force: true });
  }
});

test(journalTestName('candidate journal records ancestor-path A-to-B-to-A rebinding'), async () => {
  if (nestedJournalRejected()) return;
  const outer = mkdtempSync(join(tmpdir(), 'candidate-journal-ancestor-'));
  const moved = `${outer}-moved`;
  const root = join(outer, 'stable', 'candidate');
  mkdirSync(join(root, 'docs'), { recursive: true });
  writeFileSync(join(root, 'docs', 'contract.md'), 'A\n');
  const journal = createCandidateJournal(root);
  try {
    await journal.barrier();
    renameSync(outer, moved);
    mkdirSync(join(root, 'docs'), { recursive: true });
    writeFileSync(join(root, 'docs', 'contract.md'), 'B\n');
    assert.equal(readFileSync(join(root, 'docs', 'contract.md'), 'utf8'), 'B\n');
    rmSync(outer, { recursive: true });
    renameSync(moved, outer);
    await journal.barrier();
    assert.ok(journal.drift().events.includes('.'));
  } finally {
    await journal.close();
    rmSync(outer, { recursive: true, force: true });
    rmSync(moved, { recursive: true, force: true });
  }
});

test(journalTestName('candidate journal records persistent source drift and ignores generated stores'), async () => {
  if (nestedJournalRejected()) return;
  const root = fixture();
  const journal = createCandidateJournal(root);
  try {
    await journal.barrier();
    mkdirSync(join(root, 'target'));
    writeFileSync(join(root, 'target', 'artifact'), 'generated\n');
    writeFileSync(join(root, '.coverage'), 'generated\n');
    writeFileSync(join(root, 'coverage.json'), '{}\n');
    writeFileSync(join(root, 'coverage.out'), 'mode: atomic\n');
    writeFileSync(join(root, '.forge-coverage-lock-candidate-test'), 'lock\n');
    writeFileSync(join(root, '.forge-coverage-artifact.lock'), 'lock\n');
    mkdirSync(join(root, '.forge-coverage-backup-test'));
    await journal.barrier();
    assert.deepEqual(journal.drift(), { error: null, events: [] });
    writeFileSync(join(root, 'docs', 'contract.md'), 'changed\n');
    await journal.barrier();
    assert.ok(journal.drift().events.includes('docs/contract.md'));
  } finally {
    await journal.close();
    rmSync(root, { recursive: true, force: true });
  }
});

test(journalTestName('candidate journal fails closed when its helper dies'), async () => {
  if (nestedJournalRejected()) return;
  const root = fixture();
  const journal = createCandidateJournal(root);
  try {
    await journal.barrier();
    process.kill(helperPid(root), 'SIGKILL');
    await assert.rejects(journal.barrier(), /exited unexpectedly|write|closed/);
    assert.ok(journal.drift().error instanceof Error);
  } finally {
    await journal.close();
    rmSync(root, { recursive: true, force: true });
  }
});

test(journalTestName('candidate journal CLOSE absorbs an asynchronous helper pipe error'), async () => {
  if (nestedJournalRejected()) return;
  const root = fixture();
  const journal = createCandidateJournal(root);
  try {
    await journal.barrier();
    const pid = helperPid(root);
    process.kill(pid, 'SIGKILL');
    const blocked = new Int32Array(new SharedArrayBuffer(4));
    Atomics.wait(blocked, 0, 0, 50);
    await journal.close();
    await journal.close();
    assert.equal(await waitForDead(pid), true);
  } finally {
    await journal.close();
    rmSync(root, { recursive: true, force: true });
  }
});

test(journalTestName('candidate journal CLOSE reaps its helper'), async () => {
  if (nestedJournalRejected()) return;
  const root = fixture();
  const journal = createCandidateJournal(root);
  try {
    await journal.barrier();
    const pid = helperPid(root);
    await journal.close();
    assert.equal(await waitForDead(pid), true);
    assert.deepEqual(journal.drift(), { error: null, events: [] });
  } finally {
    await journal.close();
    rmSync(root, { recursive: true, force: true });
  }
});

test(journalTestName('awaited candidate journal CLOSE does not accumulate inotify instances'), async () => {
  if (nestedJournalRejected()) return;
  const root = fixture();
  try {
    for (let index = 0; index < 132; index += 1) {
      const journal = createCandidateJournal(root);
      await journal.barrier();
      await journal.close();
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('nested rootless mount cannot impersonate the journal interpreter', {
  skip: process.platform !== 'linux',
}, (t) => {
  const result = runMountProbe(t, 'pre', true);
  if (!result) return;
  assert.equal(result.ok, false);
  assert.equal(result.attacked, false);
  assert.match(result.error, /Python interpreter is unsafe/);
});

test('candidate journal rejects every non-initial user namespace', {
  skip: process.platform !== 'linux',
}, (t) => {
  const result = runMountProbe(t, 'namespace', false);
  if (!result) return;
  assert.equal(result.ok, false);
  assert.equal(result.attacked, false);
  assert.match(result.error, /Python interpreter is unsafe/);
});
