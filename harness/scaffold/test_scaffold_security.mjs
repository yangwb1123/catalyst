import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  chmodSync,
  existsSync,
  linkSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import {
  PROJECT_INSTANCE_FILES,
  renderScaffoldState,
  scaffold,
  SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';
import {
  removedFiles, run as runUpgrade,
} from './forge-upgrade.mjs';
import {
  assertSafeSourceProjection,
  readFileNoFollow,
  releaseFileExclusiveClaim,
  snapshotFileNoFollow,
  writeFileExclusiveNoFollow,
  writeFileNoFollow,
} from './scaffold-fs.mjs';

const SCAFFOLD_DIR = dirname(fileURLToPath(import.meta.url));
const SOURCE_ROOT = dirname(dirname(SCAFFOLD_DIR));
const FS_MODULE_URL = pathToFileURL(join(SCAFFOLD_DIR, 'scaffold-fs.mjs')).href;

function scaffoldProject(t, suffix) {
  const root = mkdtempSync(join(tmpdir(), `forge-scaffold-security-${suffix}-`));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  scaffold({
    target,
    name: `security-${suffix}`,
    mode: 'balanced',
    lifecycle: 'mvp',
    force: false,
  });
  return { root, target };
}

test('source snapshots reject in-place changes after their opening metadata', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'forge-stable-source-read-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const source = join(root, 'source.mjs');
  const initial = Buffer.from('// initial source bytes\n');
  const replacement = Buffer.from('// replacement source bytes with a new size\n');
  writeFileSync(source, initial);
  assert.throws(() => snapshotFileNoFollow(
    source, 'mutable source fixture', () => writeFileSync(source, replacement),
  ), /changed file contents.*mutable source fixture/i);
  assert.deepEqual(readFileSync(source), replacement);
});

test('upgrade rejects a hardlinked governed target without changing its outside inode', (t) => {
  const { root, target } = scaffoldProject(t, 'target-hardlink');
  const destination = join(target, 'harness', 'gate.mjs');
  const outside = join(root, 'outside-gate.mjs');
  const original = Buffer.from('// outside target bytes\n');
  writeFileSync(outside, original);
  rmSync(destination);
  linkSync(outside, destination);

  assert.throws(
    () => runUpgrade({
      from: SOURCE_ROOT, target, apply: true, backup: false, prune: false,
    }),
    /unsafe hardlink path.*gate\.mjs/i,
  );
  assert.ok(readFileSync(outside).equals(original));
});

test('changed-file publication rejects a hardlink added after preparation', (t) => {
  const { root, target } = scaffoldProject(t, 'changed-hardlink-race');
  const destination = join(target, 'harness', 'gate.mjs');
  const outside = join(root, 'outside-gate.mjs');
  const drift = Buffer.from('// pre-transaction changed bytes\n');
  writeFileSync(destination, drift);

  assert.throws(
    () => runUpgrade(
      { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false },
      new Date(),
      { afterClassification() { linkSync(destination, outside); } },
    ),
    /refusing changed file path for changed target.*gate\.mjs/i,
  );
  assert.ok(readFileSync(destination).equals(drift));
  assert.ok(readFileSync(outside).equals(drift), 'staged successor must never truncate the alias');
});

function assertRelocatedParentFails(t, kind) {
  const { root, target } = scaffoldProject(t, `${kind}-parent-race`);
  const relative = kind === 'changed'
    ? join('harness', 'gate.mjs') : join('harness', 'retired-tool.mjs');
  const destination = join(target, relative);
  const prior = Buffer.from(`// ${kind} bytes in detached parent\n`);
  writeFileSync(destination, prior);
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  if (kind === 'removed') {
    const copied = JSON.parse(readFileSync(statePath, 'utf8')).copied;
    writeFileSync(statePath, renderScaffoldState([...copied, relative]));
  }
  const stateBefore = readFileSync(statePath);
  const detached = join(root, `detached-${kind}-harness`);
  const replacement = Buffer.from(`// replacement ${kind} parent bytes\n`);
  assert.throws(
    () => runUpgrade(
      { from: SOURCE_ROOT, target, apply: true, backup: false, prune: kind === 'removed' },
      new Date(),
      { afterClassification() {
        renameSync(dirname(destination), detached);
        mkdirSync(dirname(destination));
        writeFileSync(destination, replacement);
      } },
    ),
    /refusing changed directory boundary/,
  );
  assert.ok(readFileSync(destination).equals(replacement));
  assert.ok(readFileSync(join(detached, relative.split('/').at(-1))).equals(prior));
  assert.ok(readFileSync(statePath).equals(stateBefore));
}

test('changed-file transaction rejects a relocated parent', (t) => {
  assertRelocatedParentFails(t, 'changed');
});

test('prune transaction rejects a relocated parent', (t) => {
  assertRelocatedParentFails(t, 'removed');
});

test('failed commit restores changed, pruned, project-instance and ledger paths', (t) => {
  const { target } = scaffoldProject(t, 'complete-rollback');
  const gate = join(target, 'harness', 'gate.mjs');
  const gatePrior = Buffer.from('// changed bytes requiring rollback\n');
  writeFileSync(gate, gatePrior);
  const retiredRel = join('harness', 'retired-tool.mjs');
  const retired = join(target, retiredRel);
  const retiredPrior = Buffer.from('// retired bytes requiring rollback\n');
  writeFileSync(retired, retiredPrior);
  const projectInstance = join(target, PROJECT_INSTANCE_FILES[0]);
  rmSync(projectInstance);
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const copied = JSON.parse(readFileSync(statePath, 'utf8')).copied;
  writeFileSync(statePath, renderScaffoldState([...copied, retiredRel]));
  const statePrior = readFileSync(statePath);

  assert.throws(
    () => runUpgrade(
      { from: SOURCE_ROOT, target, apply: true, backup: false, prune: true },
      new Date(),
      { afterScaffoldStateWrite() { throw new Error('forced complete rollback'); } },
    ),
    /forced complete rollback/,
  );
  assert.ok(readFileSync(gate).equals(gatePrior));
  assert.ok(readFileSync(retired).equals(retiredPrior));
  assert.equal(existsSync(projectInstance), false);
  assert.ok(readFileSync(statePath).equals(statePrior));
});

test('hardlinked scaffold state is rejected for both read and write', (t) => {
  const { root, target } = scaffoldProject(t, 'state-hardlink');
  const state = join(target, SCAFFOLD_STATE_FILE);
  const outside = join(root, 'outside-state.json');
  const original = Buffer.from('{"version":1,"copied":[]}\n');
  writeFileSync(outside, original);
  rmSync(state);
  linkSync(outside, state);

  assert.throws(
    () => runUpgrade({
      from: SOURCE_ROOT, target, apply: false, backup: true, prune: false,
    }),
    /unsafe hardlink path.*scaffold-state\.json/i,
  );
  assert.throws(
    () => writeFileNoFollow(state, '{"version":2}\n', SCAFFOLD_STATE_FILE),
    /unsafe hardlink path.*scaffold-state\.json/i,
  );
  assert.ok(readFileSync(outside).equals(original));
});

test('upgrade trusts only exact safe ledger modes and never normalizes unsafe input', (t) => {
  const { target } = scaffoldProject(t, 'state-mode');
  const state = join(target, SCAFFOLD_STATE_FILE);
  const original = readFileSync(state);
  for (const mode of [0o600, 0o640, 0o644]) {
    chmodSync(state, mode);
    const safe = runUpgrade({
      from: SOURCE_ROOT, target, apply: true, backup: false, prune: false,
    });
    assert.equal(safe.stateUpdated, false);
    assert.equal(lstatSync(state).mode & 0o7777, mode);
    assert.ok(readFileSync(state).equals(original));

    const copied = JSON.parse(original).copied;
    writeFileSync(state, renderScaffoldState([...copied, 'harness/retired-mode-test.mjs']));
    chmodSync(state, mode);
    const updated = runUpgrade({
      from: SOURCE_ROOT, target, apply: true, backup: false, prune: true,
    });
    assert.equal(updated.stateUpdated, true);
    assert.equal(lstatSync(state).mode & 0o7777, mode);
    assert.ok(readFileSync(state).equals(original));
  }

  const gate = join(target, 'harness', 'gate.mjs');
  const drift = Buffer.from('// unsafe ledger must preempt target writes\n');
  writeFileSync(gate, drift);
  for (const mode of [0o664, 0o646, 0o400, 0o4664]) {
    chmodSync(state, mode);
    assert.throws(
      () => runUpgrade({
        from: SOURCE_ROOT, target, apply: true, backup: false, prune: false,
      }),
      /scaffold-state\.json: unsafe mode/i,
    );
    assert.equal(lstatSync(state).mode & 0o7777, mode);
    assert.ok(readFileSync(state).equals(original));
    assert.ok(readFileSync(gate).equals(drift));
  }
});

test('upgrade rejects unknown, duplicate and noncanonical ledger schemas before writes', (t) => {
  const { target } = scaffoldProject(t, 'state-schema');
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const original = JSON.parse(readFileSync(statePath, 'utf8'));
  const gate = join(target, 'harness', 'gate.mjs');
  const drift = Buffer.from('// malformed ledger must preempt target writes\n');
  writeFileSync(gate, drift);
  const cases = [
    [{ ...original, version: 2 }, /exact .*version: 1/i],
    [{ ...original, extra: true }, /exact .*schema/i],
    [{ ...original, copied: [...original.copied, original.copied[0]] }, /must be unique/i],
    [{ ...original, copied: [...original.copied, 'harness\\gate.mjs'] }, /unsafe path/i],
    [{ version: 1, copied: 'harness/gate.mjs' }, /exact .*copied/i],
  ];
  for (const [ledger, error] of cases) {
    const encoded = Buffer.from(`${JSON.stringify(ledger, null, 2)}\n`);
    writeFileSync(statePath, encoded);
    chmodSync(statePath, 0o600);
    assert.throws(
      () => runUpgrade({
        from: SOURCE_ROOT, target, apply: true, backup: false, prune: false,
      }),
      error,
    );
    assert.ok(readFileSync(statePath).equals(encoded));
    assert.ok(readFileSync(gate).equals(drift));
  }
});

test('forced init preflights every existing leaf before changing an earlier file', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'forge-init-preflight-hardlink-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const early = join(target, '.agent', 'agents', 'architect.md');
  mkdirSync(dirname(early), { recursive: true });
  const earlyBytes = Buffer.from('user-owned early bytes\n');
  writeFileSync(early, earlyBytes);

  const outside = join(root, 'outside-readme.md');
  const outsideBytes = Buffer.from('outside inode must remain unchanged\n');
  writeFileSync(outside, outsideBytes);
  linkSync(outside, join(target, 'README.md'));

  assert.throws(
    () => scaffold({
      target,
      name: 'force-preflight',
      mode: 'balanced',
      lifecycle: 'mvp',
      force: true,
    }),
    /unsafe hardlink path.*README\.md/i,
  );
  assert.ok(readFileSync(early).equals(earlyBytes), 'late rejection must precede early overwrite');
  assert.ok(readFileSync(outside).equals(outsideBytes), 'external hardlink inode must not change');
});

test('failed exclusive staging never publishes a partial destination', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'forge-exclusive-staging-failure-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const destination = join(root, 'governed.txt');

  assert.throws(
    () => writeFileExclusiveNoFollow(destination, null, 'governed staging failure'),
    /data.*string|buffer|typedarray|dataview/i,
  );
  assert.equal(existsSync(destination), false);
  assert.deepEqual(
    readdirSync(root).filter((name) => name.startsWith('.forge-exclusive-')),
    [],
  );
});

test('exclusive publication transfers a descriptor-backed claim before cleanup', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'forge-exclusive-owned-publication-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const destination = join(root, 'governed.txt');
  const claim = writeFileExclusiveNoFollow(
    destination, Buffer.from('owned publication\n'), 'owned publication', 0o600,
  );
  assert.notEqual(claim, null);
  assert.equal(lstatSync(destination).nlink, 2);
  assert.equal(existsSync(claim.sentinel), true);
  releaseFileExclusiveClaim(claim, 'owned publication');
  assert.equal(lstatSync(destination).nlink, 1);
  assert.equal(existsSync(claim.sentinel), false);
  assert.deepEqual(
    readdirSync(root).filter((name) => name.startsWith('.forge-exclusive-')), [],
  );
});

test('--prune rejects normalized, Win32-trimmed, and portable current aliases', (t) => {
  const cases = [
    ['harness/sub/../gate.mjs', /unsafe path.*harness\/sub\/\.\.\/gate\.mjs/],
    ['harness//gate.mjs', /unsafe path.*harness\/\/gate\.mjs/],
    ['harness/.. /README.md', /unsafe path.*harness\/\.\. \/README\.md/],
    ['harness/. /gate.mjs', /unsafe path.*harness\/\. \/gate\.mjs/],
    ['harness/GATE.mjs', /aliases current governed path.*gate\.mjs/i],
    ['harness/gate.mjs.', /unsafe path.*gate\.mjs\./i],
  ];
  for (const [alias, error] of cases) {
    const { target } = scaffoldProject(t, `state-alias-${alias.length}`);
    const gate = join(target, 'harness', 'gate.mjs');
    const gateBytes = readFileSync(gate);
    const readme = readFileSync(join(target, 'README.md'));
    const statePath = join(target, SCAFFOLD_STATE_FILE);
    const state = JSON.parse(readFileSync(statePath, 'utf8'));
    state.copied.push(alias);
    writeFileSync(statePath, `${JSON.stringify(state, null, 2)}\n`);

    for (const apply of [false, true]) {
      assert.throws(
        () => runUpgrade({
          from: SOURCE_ROOT, target, apply, backup: false, prune: true,
        }),
        error,
        `state alias ${alias} must fail closed (${apply ? 'apply' : 'dry'})`,
      );
    }
    assert.ok(readFileSync(gate).equals(gateBytes));
    assert.ok(readFileSync(join(target, 'README.md')).equals(readme));
  }
});

test('removed-file discovery rejects a filesystem-identity alias of a current asset', (t) => {
  const { target } = scaffoldProject(t, 'retired-hardlink-alias');
  const alias = join('harness', 'retired-hardlink.mjs');
  const gate = join(target, 'harness', 'gate.mjs');
  linkSync(gate, join(target, alias));
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  state.copied.push(alias);
  writeFileSync(statePath, `${JSON.stringify(state, null, 2)}\n`);

  assert.throws(
    () => removedFiles(SOURCE_ROOT, target),
    /aliases current governed path.*gate\.mjs/i,
  );
  assert.equal(existsSync(gate), true);
  assert.equal(existsSync(join(target, alias)), true);
});

test('--prune rejects two retired spellings that can alias one host file', (t) => {
  const { target } = scaffoldProject(t, 'retired-case-alias');
  const retired = join('harness', 'retired-tool.mjs');
  const retiredBytes = Buffer.from('// retired bytes must not be partly deleted\n');
  writeFileSync(join(target, retired), retiredBytes);
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  state.copied.push(retired, join('harness', 'RETIRED-TOOL.mjs'));
  writeFileSync(statePath, `${JSON.stringify(state, null, 2)}\n`);

  for (const apply of [false, true]) {
    assert.throws(
      () => runUpgrade({
        from: SOURCE_ROOT, target, apply, backup: false, prune: true,
      }),
      /aliases retired path/i,
    );
    assert.ok(readFileSync(join(target, retired)).equals(retiredBytes));
  }
});

test('upgrade rejects a hardlinked backup leaf before overwriting governed bytes', (t) => {
  const { root, target } = scaffoldProject(t, 'backup-hardlink');
  const now = new Date('2026-07-27T12:00:00.000Z');
  const gate = join(target, 'harness', 'gate.mjs');
  const agents = join(target, '.agent', 'AGENTS.md');
  const gateDrift = Buffer.from('// target gate drift requiring a backup\n');
  const agentsDrift = Buffer.from('// earlier target drift must remain untouched\n');
  writeFileSync(gate, gateDrift);
  writeFileSync(agents, agentsDrift);

  const backupGate = join(
    target,
    '.forge',
    'upgrade-backup',
    '2026-07-27T12-00-00.000Z',
    'harness',
    'gate.mjs',
  );
  mkdirSync(dirname(backupGate), { recursive: true });
  const outside = join(root, 'outside-backup.mjs');
  const original = Buffer.from('// outside backup bytes\n');
  writeFileSync(outside, original);
  linkSync(outside, backupGate);

  assert.throws(
    () => runUpgrade(
      { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false },
      now,
    ),
    /unsafe hardlink path.*backup.*gate\.mjs/i,
  );
  assert.ok(readFileSync(outside).equals(original));
  assert.ok(readFileSync(agents).equals(agentsDrift), 'later backup rejection must precede all writes');
  assert.ok(readFileSync(gate).equals(gateDrift), 'backup rejection must precede target overwrite');
});

test('special source/destination leaves fail before a FIFO can block open', (t) => {
  if (process.platform === 'win32') {
    t.skip('POSIX FIFO regression is not applicable on Windows');
    return;
  }
  const root = mkdtempSync(join(tmpdir(), 'forge-scaffold-security-fifo-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const fifo = join(root, 'governed-fifo');
  const made = spawnSync('mkfifo', [fifo], { encoding: 'utf8' });
  if (made.status !== 0) {
    t.skip(`mkfifo unavailable: ${made.error?.message ?? made.stderr}`);
    return;
  }

  assert.throws(
    () => assertSafeSourceProjection(root, ['governed-fifo']),
    /unsafe non-file path.*governed-fifo/i,
  );
  const childCode = `
    import { readFileNoFollow, writeFileNoFollow } from ${JSON.stringify(FS_MODULE_URL)};
    const path = process.argv[1];
    for (const operation of [
      () => readFileNoFollow(path, 'FIFO read'),
      () => writeFileNoFollow(path, 'replacement', 'FIFO write'),
    ]) {
      try {
        operation();
        process.exit(2);
      } catch (err) {
        if (!/unsafe non-file path/.test(String(err?.message))) {
          console.error(err);
          process.exit(3);
        }
      }
    }
  `;
  const checked = spawnSync(process.execPath, ['--input-type=module', '-e', childCode, fifo], {
    encoding: 'utf8',
    timeout: 2000,
  });
  assert.equal(checked.error, undefined, `FIFO guard must not block: ${checked.error?.message}`);
  assert.equal(
    checked.status,
    0,
    `FIFO guard child failed; stdout=${checked.stdout} stderr=${checked.stderr}`,
  );

  // Direct calls remain covered too; both throw synchronously before openSync.
  assert.throws(() => readFileNoFollow(fifo, 'FIFO read'), /unsafe non-file path/);
  assert.throws(
    () => writeFileNoFollow(fifo, 'replacement', 'FIFO write'),
    /unsafe non-file path/,
  );
});
