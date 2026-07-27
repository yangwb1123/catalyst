import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  existsSync,
  linkSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import {
  scaffold,
  SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';
import {
  removedFiles, run as runUpgrade,
} from './forge-upgrade.mjs';
import {
  assertSafeSourceProjection,
  readFileNoFollow,
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
