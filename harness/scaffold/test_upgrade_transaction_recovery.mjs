import test from 'node:test';
import assert from 'node:assert/strict';
import {
  chmodSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync,
  readdirSync, renameSync, rmSync, writeFileSync,
} from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { basename, dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { renderScaffoldState, scaffold, SCAFFOLD_STATE_FILE } from './forge-init.mjs';
import { run, sourceProvenance } from './forge-upgrade.mjs';
import { UPGRADE_TRANSACTION_FILE } from './upgrade-transaction-journal.mjs';

const SCAFFOLD_DIR = dirname(fileURLToPath(import.meta.url));
const SOURCE_ROOT = dirname(dirname(SCAFFOLD_DIR));
const UPGRADE_URL = pathToFileURL(join(SCAFFOLD_DIR, 'forge-upgrade.mjs')).href;
const NOW = new Date('2026-08-21T15:00:00.000Z');
const BACKUP_STAMP = '2026-08-21T15-00-00.000Z';
const OWNED_PACKAGE = join('harness', 'decision_capsule_contract');
function project(t, suffix) {
  const root = mkdtempSync(join(tmpdir(), `forge-upgrade-recovery-${suffix}-`));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  scaffold({
    target, name: `recovery-${suffix}`, mode: 'balanced', lifecycle: 'mvp', force: false,
  });
  return { root, target };
}
function transactionArtifacts(root, found = []) {
  if (!existsSync(root)) return found;
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.name.startsWith('.forge-upgrade-txn-')
        || entry.name.startsWith('.forge-upgrade-owned-')
        || entry.name === '.forge-upgrade-owner'
        || entry.name.startsWith('.forge-upgrade-transaction-v1.json')) found.push(path);
    if (entry.isDirectory()) transactionArtifacts(path, found);
  }
  return found;
}
function transaction(target) {
  return JSON.parse(
    readFileSync(join(target, UPGRADE_TRANSACTION_FILE), 'utf8').split('\n', 1)[0],
  );
}
function prepareOwnedPackageCrash(target) {
  rmSync(join(target, OWNED_PACKAGE), { recursive: true });
  const crashed = crashUpgrade(target, 'afterAddedReservation');
  assert.equal(crashed.status, null, `${crashed.stdout}\n${crashed.stderr}`);
  assert.equal(crashed.signal, 'SIGKILL', `${crashed.stdout}\n${crashed.stderr}`);
}
function crashUpgrade(target, hook) {
  const child = `
    import { run } from ${JSON.stringify(UPGRADE_URL)};
    const [source, target, hook] = process.argv.slice(1);
    run(
      { from: source, target, apply: true, backup: true, prune: false },
      new Date(${JSON.stringify(NOW.toISOString())}),
      { [hook]() { process.kill(process.pid, 'SIGKILL'); } },
    );
  `;
  return spawnSync(
    process.execPath,
    ['--input-type=module', '-e', child, SOURCE_ROOT, target, hook],
    { encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 },
  );
}
function prepareCrashDrift(target) {
  const added = join('harness', 'gate.mjs');
  const changed = join('.agent', 'AGENTS.md');
  rmSync(join(target, added));
  const prior = Buffer.from('// changed before interrupted upgrade\n');
  writeFileSync(join(target, changed), prior);
  return { added, changed, prior };
}
function assertRecoveredTarget(target, drift) {
  for (const rel of [drift.added, drift.changed]) {
    assert.deepEqual(readFileSync(join(target, rel)), readFileSync(join(SOURCE_ROOT, rel)));
    assert.equal(lstatSync(join(target, rel)).nlink, 1);
  }
  const backup = join(
    target, '.forge', 'upgrade-backup', BACKUP_STAMP, drift.changed,
  );
  assert.deepEqual(readFileSync(backup), drift.prior);
  assert.equal(lstatSync(backup).nlink, 1);
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), false);
  assert.deepEqual(transactionArtifacts(target), []);
}
for (const hook of [
  'afterAddedReservation', 'afterScaffoldStateWrite', 'afterTransactionDurability',
  'afterTransactionCommit',
  'afterTransactionFinishMarker',
]) {
  test(`SIGKILL at ${hook} is recovered and retried from the durable journal`, (t) => {
    const { target } = project(t, hook);
    const drift = prepareCrashDrift(target);
    const crashed = crashUpgrade(target, hook);
    assert.equal(crashed.status, null, `${crashed.stdout}\n${crashed.stderr}`);
    assert.equal(crashed.signal, 'SIGKILL', `${crashed.stdout}\n${crashed.stderr}`);
    assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), true);

    const retried = run(
      { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW,
    );
    assert.equal(retried.applied, true);
    assertRecoveredTarget(target, drift);
  });
}

test('a late backup collision retracts every earlier tool-created backup', (t) => {
  const { target } = project(t, 'backup-rollback');
  const changed = [join('.agent', 'AGENTS.md'), join('harness', 'gate.mjs')];
  const prior = new Map();
  for (const rel of changed) {
    const bytes = Buffer.from(`// prior ${rel}\n`);
    writeFileSync(join(target, rel), bytes);
    prior.set(rel, bytes);
  }
  const backupRoot = join(target, '.forge', 'upgrade-backup', BACKUP_STAMP);
  const collision = join(backupRoot, changed.at(-1));
  const collisionBytes = Buffer.from('// concurrent backup owner\n');
  assert.throws(
    () => run(
      { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW,
      { afterBackupReservation({ sourceRel }) {
        if (sourceRel !== changed[0]) return;
        mkdirSync(dirname(collision), { recursive: true });
        writeFileSync(collision, collisionBytes);
      } },
    ),
    /EEXIST|file already exists|expected an absent destination/i,
  );
  assert.deepEqual(readFileSync(collision), collisionBytes);
  assert.equal(existsSync(join(backupRoot, changed[0])), false);
  for (const rel of changed) assert.deepEqual(readFileSync(join(target, rel)), prior.get(rel));
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), false);
  assert.deepEqual(transactionArtifacts(target), []);
});

test('double replacement preserves both unknown inodes and its recovery journal', (t) => {
  const { target } = project(t, 'double-replacement');
  const rel = join('.agent', 'AGENTS.md');
  const live = join(target, rel);
  const prior = Buffer.from('// transaction prior\n');
  const first = Buffer.from('// first concurrent replacement\n');
  const second = Buffer.from('// second concurrent replacement\n');
  writeFileSync(live, prior);
  const child = `
    import { unlinkSync, writeFileSync } from 'node:fs';
    import { run } from ${JSON.stringify(UPGRADE_URL)};
    const [source, target, rel] = process.argv.slice(1);
    const live = target + '/' + rel;
    run({ from: source, target, apply: true, backup: true, prune: false },
      new Date(${JSON.stringify(NOW.toISOString())}), {
        beforePriorDetach({ rel: found }) {
          if (found === rel) { unlinkSync(live); writeFileSync(live, ${JSON.stringify(first.toString())}); }
        },
        afterPriorDetach({ rel: found }) {
          if (found === rel) writeFileSync(live, ${JSON.stringify(second.toString())});
        },
      });
  `;
  const failed = spawnSync(process.execPath,
    ['--input-type=module', '-e', child, SOURCE_ROOT, target, rel], { encoding: 'utf8' });
  assert.notEqual(failed.status, 0, `${failed.stdout}\n${failed.stderr}`);
  const plan = transaction(target).entries.find((item) => item.kind === 'changed' && item.rel === rel);
  const stage = join(dirname(live), plan.stage_name);
  const priorLive = join(stage, readdirSync(stage).find((name) => name.startsWith('prior-live-')));
  assert.deepEqual(readFileSync(live), second);
  assert.deepEqual(readFileSync(priorLive), first);
  assert.deepEqual(readFileSync(join(stage, 'prior')), prior);
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW,
  ), /preserved concurrent replacement/);
  assert.deepEqual(readFileSync(live), second);
  assert.deepEqual(readFileSync(priorLive), first);
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), true);
});

test('committed recovery preserves an unknown stage-next replacement', (t) => {
  const { target } = project(t, 'committed-stage-next-replacement');
  const drift = prepareCrashDrift(target);
  assert.equal(crashUpgrade(target, 'afterTransactionCommit').signal, 'SIGKILL');
  const plan = transaction(target).entries.find(
    (item) => item.kind === 'changed' && item.rel === drift.changed,
  );
  const next = join(dirname(join(target, drift.changed)), plan.stage_name, 'next');
  const unknown = Buffer.from('// unknown stage next\n');
  rmSync(next);
  writeFileSync(next, unknown);
  chmodSync(next, 0o600);
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW,
  ), /upgrade stage next/i);
  assert.deepEqual(readFileSync(next), unknown);
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), true);
});

test('stage cleanup rechecks a file after recovery validation', (t) => {
  const { target } = project(t, 'stage-file-cleanup-race');
  prepareCrashDrift(target);
  assert.equal(crashUpgrade(target, 'afterTransactionCommit').signal, 'SIGKILL');
  const document = transaction(target);
  let stageName;
  const unknown = Buffer.from('late stage file replacement\n');
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW,
    { beforeUpgradeStageFileCleanup({ path }) {
      if (stageName !== undefined) return;
      stageName = basename(path);
      rmSync(join(path, 'next'));
      writeFileSync(join(path, 'next'), unknown);
      chmodSync(join(path, 'next'), 0o600);
    } },
  ), /upgrade stage next/i);
  const plan = document.entries.find((item) => item.stage_name === stageName);
  const stage = join(dirname(join(target, plan.rel)), plan.stage_name);
  assert.deepEqual(readFileSync(join(stage, 'next')), unknown);
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), true);
});

test('stage cleanup preserves a replacement directory inode', (t) => {
  const { target } = project(t, 'stage-directory-cleanup-race');
  prepareCrashDrift(target);
  assert.equal(crashUpgrade(target, 'afterTransactionCommit').signal, 'SIGKILL');
  const document = transaction(target);
  let stageName;
  let unknownIdentity;
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW,
    { beforeUpgradeStageDirectoryDetach({ path }) {
      if (stageName !== undefined) return;
      stageName = basename(path);
      rmSync(path, { recursive: true });
      mkdirSync(path);
      const stat = lstatSync(path, { bigint: true });
      unknownIdentity = { dev: stat.dev, ino: stat.ino };
    } },
  ), /preserved unknown upgrade stage directory/i);
  const plan = document.entries.find((item) => item.stage_name === stageName);
  const parent = dirname(join(target, plan.rel));
  const prefix = `${stageName}.removing-`;
  const moved = readdirSync(parent).find((name) => name.startsWith(prefix));
  const preserved = moved === undefined ? join(parent, stageName) : join(parent, moved);
  const preservedStat = lstatSync(preserved, { bigint: true });
  assert.deepEqual({ dev: preservedStat.dev, ino: preservedStat.ino }, unknownIdentity);
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), true);
});

test('transaction controls stay on their pinned agent directory during replacement', (t) => {
  const { root, target } = project(t, 'control-parent-replacement');
  writeFileSync(join(target, '.agent', 'AGENTS.md'), '// force an upgrade\n');
  const original = join(root, 'original-agent');
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW,
    { afterTransactionJournalWrite() {
      renameSync(join(target, '.agent'), original);
      mkdirSync(join(target, '.agent'), { mode: 0o700 });
    } },
  ), /changed.*control directory/i);
  assert.deepEqual(transactionArtifacts(original), []);
  assert.deepEqual(transactionArtifacts(join(target, '.agent')), []);
});

test('an unknown legacy next control is preserved and never claimed', (t) => {
  const { target } = project(t, 'unknown-next-control');
  const path = join(target, `${UPGRADE_TRANSACTION_FILE}.next`);
  const bytes = Buffer.from('unknown control owner\n');
  writeFileSync(path, bytes);
  chmodSync(path, 0o600);
  run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW);
  assert.deepEqual(readFileSync(path), bytes);
});

test('control cleanup quarantines before deciding which inode to remove', (t) => {
  const { target } = project(t, 'control-cleanup-replacement');
  const unknown = Buffer.from('unknown transaction journal\n');
  let replaced = false;
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW,
    { beforeTransactionControlDetach({ name, path }) {
      if (name !== 'journal' || replaced) return;
      replaced = true;
      rmSync(path);
      writeFileSync(path, unknown);
      chmodSync(path, 0o600);
    } },
  ), /unknown replacement preserved/i);
  assert.deepEqual(readFileSync(join(target, UPGRADE_TRANSACTION_FILE)), unknown);
  assert.equal(existsSync(join(target, `${UPGRADE_TRANSACTION_FILE}.finished`)), true);
});

test('target root identity is pinned before destination directory creation', (t) => {
  const { root, target } = project(t, 'target-root-replacement');
  rmSync(join(target, OWNED_PACKAGE), { recursive: true });
  const original = join(root, 'original-project');
  const replacement = join(root, 'replacement-project');
  const sentinel = Buffer.from('replacement root owner\n');
  const child = `
    import { mkdirSync, renameSync, writeFileSync } from 'node:fs';
    import { run } from ${JSON.stringify(UPGRADE_URL)};
    const [source, target, original] = process.argv.slice(1);
    run({ from: source, target, apply: true, backup: true, prune: false },
      new Date(${JSON.stringify(NOW.toISOString())}), { afterTransactionStart() {
        renameSync(target, original); mkdirSync(target);
        writeFileSync(target + '/sentinel.txt', ${JSON.stringify(sentinel.toString())});
      } });
  `;
  const failed = spawnSync(process.execPath,
    ['--input-type=module', '-e', child, SOURCE_ROOT, target, original], { encoding: 'utf8' });
  assert.notEqual(failed.status, 0, `${failed.stdout}\n${failed.stderr}`);
  assert.match(`${failed.stdout}\n${failed.stderr}`, /changed target root|changed directory boundary/i);
  assert.deepEqual(readdirSync(target), ['sentinel.txt']);
  renameSync(target, replacement);
  renameSync(original, target);
  run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW);
  assert.deepEqual(readFileSync(join(replacement, 'sentinel.txt')), sentinel);
  assert.deepEqual(
    readFileSync(join(target, OWNED_PACKAGE, '__init__.py')),
    readFileSync(join(SOURCE_ROOT, OWNED_PACKAGE, '__init__.py')),
  );
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), false);
});

test('classification cannot be applied to a replacement target root', (t) => {
  const { root, target } = project(t, 'classification-root-replacement');
  const replacement = join(root, 'replacement-project');
  scaffold({
    target: replacement, name: 'replacement', mode: 'balanced', lifecycle: 'mvp', force: false,
  });
  const rel = join('harness', 'gate.mjs');
  const original = join(root, 'classified-project');
  const classifiedBytes = Buffer.from('// classified target drift\n');
  const replacementBytes = Buffer.from('// replacement unowned bytes\n');
  writeFileSync(join(target, rel), classifiedBytes);
  const statePath = join(replacement, SCAFFOLD_STATE_FILE);
  const copied = JSON.parse(readFileSync(statePath, 'utf8')).copied.filter((item) => item !== rel);
  writeFileSync(statePath, renderScaffoldState(copied));
  writeFileSync(join(replacement, rel), replacementBytes);
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW,
    { afterApplyReport() { renameSync(target, original); renameSync(replacement, target); } },
  ), /changed directory boundary|target changed/i);
  assert.deepEqual(readFileSync(join(target, rel)), replacementBytes);
  assert.deepEqual(readFileSync(join(original, rel)), classifiedBytes);
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), false);
});

test('owned-directory marker unlink is restart-idempotent through its durable proof', (t) => {
  const { target } = project(t, 'owned-directory-proof');
  prepareOwnedPackageCrash(target);
  const interrupted = crashUpgrade(target, 'afterOwnedDirectoryMarkerUnlink');
  assert.equal(interrupted.status, null, `${interrupted.stdout}\n${interrupted.stderr}`);
  assert.equal(interrupted.signal, 'SIGKILL', `${interrupted.stdout}\n${interrupted.stderr}`);
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), true);
  assert.ok(transactionArtifacts(target).some((path) => path.includes('owned-proof')));
  run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW);
  assert.deepEqual(
    readFileSync(join(target, OWNED_PACKAGE, '__init__.py')),
    readFileSync(join(SOURCE_ROOT, OWNED_PACKAGE, '__init__.py')),
  );
  assert.deepEqual(transactionArtifacts(target), []);
});

test('owned-directory quarantine never removes a concurrent replacement directory', (t) => {
  const { target } = project(t, 'owned-directory-replacement');
  prepareOwnedPackageCrash(target);
  const sentinel = Buffer.from('concurrent directory owner\n');
  run(
    { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW,
    { afterOwnedDirectoryQuarantine({ path, relative }) {
      if (relative !== OWNED_PACKAGE) return;
      mkdirSync(path);
      writeFileSync(join(path, 'concurrent.txt'), sentinel);
    } },
  );
  assert.deepEqual(readFileSync(join(target, OWNED_PACKAGE, 'concurrent.txt')), sentinel);
  assert.deepEqual(transactionArtifacts(target), []);
});

test('owned-directory random cleanup preserves an unknown deterministic quarantine name', (t) => {
  const { target } = project(t, 'owned-directory-reservation-race');
  prepareOwnedPackageCrash(target);
  const sentinel = Buffer.from('unknown quarantine owner\n');
  let unknownPath;
  run(
    { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW,
    { beforeOwnedDirectoryQuarantineReservation({ path, quarantine, relative }) {
      if (relative !== OWNED_PACKAGE) return;
      unknownPath = join(dirname(path), basename(quarantine));
      mkdirSync(unknownPath);
      writeFileSync(join(unknownPath, 'unknown.txt'), sentinel);
    } },
  );
  assert.deepEqual(readFileSync(join(unknownPath, 'unknown.txt')), sentinel);
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), false);
});

test('owned-directory final detach preserves a replacement quarantine inode', (t) => {
  const { target } = project(t, 'owned-directory-final-race');
  prepareOwnedPackageCrash(target);
  let quarantinePath;
  let unknownIdentity;
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW,
    { beforeOwnedDirectoryFinalDetach({ path, quarantine, relative }) {
      if (relative !== OWNED_PACKAGE) return;
      quarantinePath = join(dirname(path), basename(quarantine));
      rmSync(quarantinePath, { recursive: true });
      mkdirSync(quarantinePath);
      const stat = lstatSync(quarantinePath, { bigint: true });
      unknownIdentity = { dev: stat.dev, ino: stat.ino };
    } },
  ), /preserved unknown owned-directory quarantine/i);
  const prefix = `${basename(quarantinePath)}.removing-`;
  const moved = readdirSync(dirname(quarantinePath)).find((name) => name.startsWith(prefix));
  const preserved = moved === undefined ? quarantinePath : join(dirname(quarantinePath), moved);
  const preservedStat = lstatSync(preserved, { bigint: true });
  assert.deepEqual({ dev: preservedStat.dev, ino: preservedStat.ino }, unknownIdentity);
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), true);
});
test('malformed process-start metadata cannot recover a live owner journal', (t) => {
  const { target } = project(t, 'malformed-owner');
  prepareCrashDrift(target);
  const crashed = crashUpgrade(target, 'afterAddedReservation');
  assert.equal(crashed.signal, 'SIGKILL');
  const path = join(target, UPGRADE_TRANSACTION_FILE);
  const document = transaction(target);
  document.owner = { pid: process.pid, process_start: 'not-a-decimal-start-time' };
  writeFileSync(path, `${JSON.stringify(document, null, 2)}\n`);
  chmodSync(path, 0o600);
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW,
  ), /malformed scaffold upgrade transaction journal/);
  assert.equal(existsSync(path), true);
});

const JOURNAL_SHAPE_MUTATIONS = [
  ['phase', (document) => { document.phase = 'committed'; }],
  ['owner fields', (document) => { document.owner.unexpected = true; }],
  ['target fields', (document) => { delete document.target.ino; }],
  ['duplicate entries', (document) => {
    const duplicate = { ...document.entries[0] };
    duplicate.stage_name = `.forge-upgrade-txn-${document.transaction_id.slice(0, 12)}-${
      String(document.entries.length).padStart(4, '0')}`;
    document.entries.push(duplicate);
  }],
  ['cross-kind duplicate target', (document) => {
    const duplicate = { ...document.entries[0], kind: 'changed' };
    duplicate.stage_name = `.forge-upgrade-txn-${document.transaction_id.slice(0, 12)}-${
      String(document.entries.length).padStart(4, '0')}`;
    document.entries.push(duplicate);
  }],
  ['directory projection', (document) => { document.directories.reverse(); }],
];
for (const [label, mutate] of JOURNAL_SHAPE_MUTATIONS) {
  test(`journal rejects non-exact ${label}`, (t) => {
    const { target } = project(t, `journal-${label.replace(' ', '-')}`);
    prepareCrashDrift(target);
    assert.equal(crashUpgrade(target, 'afterAddedReservation').signal, 'SIGKILL');
    const path = join(target, UPGRADE_TRANSACTION_FILE);
    const document = transaction(target);
    mutate(document);
    writeFileSync(path, `${JSON.stringify(document, null, 2)}\n`);
    chmodSync(path, 0o600);
    assert.throws(() => run(
      { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW,
    ), /malformed scaffold upgrade transaction/);
    assert.equal(existsSync(path), true);
  });
}

test('source provenance distinguishes a dirty working tree from its HEAD', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'forge-upgrade-provenance-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const runGit = (...args) => spawnSync('git', args, { cwd: root, encoding: 'utf8' });
  assert.equal(runGit('init', '-q').status, 0);
  writeFileSync(join(root, 'source.txt'), 'clean\n');
  writeFileSync(join(root, '.gitignore'), '.agent/agents/ignored.md\n');
  assert.equal(runGit('add', 'source.txt', '.gitignore').status, 0);
  assert.equal(runGit(
    '-c', 'user.name=Forge Test', '-c', 'user.email=forge@example.invalid',
    'commit', '-qm', 'fixture',
  ).status, 0);
  assert.match(sourceProvenance(root), /^clean commit [a-f0-9]+$/);
  mkdirSync(join(root, '.agent', 'agents'), { recursive: true });
  writeFileSync(join(root, '.agent', 'agents', 'ignored.md'), 'ignored working bytes\n');
  assert.match(sourceProvenance(root), /^dirty working tree based on HEAD /);
  rmSync(join(root, '.agent'), { recursive: true });
  writeFileSync(join(root, 'source.txt'), 'dirty\n');
  assert.match(
    sourceProvenance(root),
    /^dirty working tree based on HEAD [a-f0-9]+; copying exact working-tree bytes$/,
  );
  writeFileSync(join(root, 'untracked.txt'), 'untracked\n');
  assert.match(sourceProvenance(root), /^dirty working tree based on HEAD /);
});
