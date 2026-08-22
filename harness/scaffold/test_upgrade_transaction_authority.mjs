import test from 'node:test';
import assert from 'node:assert/strict';
import {
  appendFileSync, chmodSync, copyFileSync, existsSync, lstatSync, mkdirSync, mkdtempSync,
  readFileSync, readdirSync, renameSync, rmSync, statSync, writeFileSync,
} from 'node:fs';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { basename, dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import {
  PROJECT_INSTANCE_FILES, scaffold,
} from './forge-init.mjs';
import {
  manifestProjection, run,
} from './forge-upgrade.mjs';
import {
  beginUpgradeTransaction, UPGRADE_TRANSACTION_FILE,
} from './upgrade-transaction-journal.mjs';
import { appendUpgradeStageIntent } from './transaction/upgrade-stage-intent-authority.mjs';

const SCAFFOLD_DIR = dirname(fileURLToPath(import.meta.url));
const SOURCE_ROOT = dirname(dirname(SCAFFOLD_DIR));
const UPGRADE_URL = pathToFileURL(join(SCAFFOLD_DIR, 'forge-upgrade.mjs')).href;
const NOW = new Date('2026-08-21T18:00:00.000Z');
const CHANGED = join('.agent', 'AGENTS.md');
const ADDED = join('harness', 'gate.mjs');

function project(t, suffix) {
  const root = mkdtempSync(join(tmpdir(), `forge-upgrade-authority-${suffix}-`));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  scaffold({ target, name: suffix, mode: 'balanced', lifecycle: 'mvp', force: false });
  return { root, target };
}

function forceChanged(target) {
  const prior = Buffer.from('// transaction authority prior\n');
  writeFileSync(join(target, CHANGED), prior);
  return prior;
}

function transaction(target) {
  return JSON.parse(
    readFileSync(join(target, UPGRADE_TRANSACTION_FILE), 'utf8').split('\n', 1)[0],
  );
}

function crash(target, hook, needle = '') {
  const child = `
    import { run } from ${JSON.stringify(UPGRADE_URL)};
    const [source, target, hook, needle] = process.argv.slice(1);
    run({ from: source, target, apply: true, backup: false, prune: false },
      new Date(${JSON.stringify(NOW.toISOString())}),
      { [hook](payload) {
        if (!needle || JSON.stringify(payload).includes(needle)) process.kill(process.pid, 'SIGKILL');
      } });
  `;
  return spawnSync(
    process.execPath,
    ['--input-type=module', '-e', child, SOURCE_ROOT, target, hook, needle],
    { encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 },
  );
}

function stageFor(target, kind, rel) {
  const entry = transaction(target).entries.find(
    (item) => item.kind === kind && item.rel === rel,
  );
  return join(dirname(join(target, rel)), entry.stage_name);
}

function preservedStageArtifact(stage, name) {
  const prefix = `${basename(stage)}.removing-`;
  const candidates = [stage, ...readdirSync(dirname(stage))
    .filter((entry) => entry.startsWith(prefix)).map((entry) => join(dirname(stage), entry))];
  const found = candidates.map((entry) => join(entry, name)).filter(existsSync);
  assert.equal(found.length, 1);
  return found[0];
}

function forgeSelfBoundStage(stage, entry, bytes) {
  const next = join(stage, 'next');
  const claim = join(stage, 'stage-claim.json');
  writeFileSync(next, bytes);
  chmodSync(next, 0o600);
  writeFileSync(claim, '');
  chmodSync(claim, 0o600);
  const stored = (stat) => ({ dev: String(stat.dev), ino: String(stat.ino) });
  const nextStat = lstatSync(next, { bigint: true });
  writeFileSync(claim, `${JSON.stringify({
    api_version: 'forgeos.scaffold-upgrade-stage-claim/v1',
    control: stored(lstatSync(claim, { bigint: true })),
    directory: stored(lstatSync(stage, { bigint: true })),
    next: {
      ...stored(nextStat), mode: 0o600,
      sha256: createHash('sha256').update(bytes).digest('hex'),
    },
    stage_name: entry.stage_name,
  })}\n`);
  return { claim, next };
}

test('SIGKILL before canonical control publication cannot wedge the next upgrade', (t) => {
  const { target } = project(t, 'private-control-kill');
  forceChanged(target);
  const killed = crash(target, 'afterTransactionControlPrivateSync');
  assert.equal(killed.signal, 'SIGKILL', `${killed.stdout}\n${killed.stderr}`);
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), false);
  const privateControls = readdirSync(join(target, '.agent')).filter(
    (name) => name.includes('.publishing-'),
  );
  assert.equal(privateControls.length, 1);
  run({ from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW);
  assert.deepEqual(readFileSync(join(target, CHANGED)), readFileSync(join(SOURCE_ROOT, CHANGED)));
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), false);
  assert.equal(existsSync(join(target, '.agent', privateControls[0])), true);
});

test('SIGKILL before a stage claim is recovered without wedging retry', (t) => {
  const { target } = project(t, 'preclaim-kill');
  forceChanged(target);
  const killed = crash(target, 'afterUpgradeStageDirectoryCreate');
  assert.equal(killed.signal, 'SIGKILL', `${killed.stdout}\n${killed.stderr}`);
  const abandoned = stageFor(target, 'changed', CHANGED);
  assert.equal(existsSync(join(abandoned, 'stage-claim.json')), false);
  run({ from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW);
  assert.deepEqual(readFileSync(join(target, CHANGED)), readFileSync(join(SOURCE_ROOT, CHANGED)));
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), false);
  assert.equal(existsSync(abandoned), false);
});

test('a torn terminal stage intent is discarded before empty-stage recovery', (t) => {
  const { target } = project(t, 'torn-stage-intent');
  forceChanged(target);
  const killed = crash(target, 'afterUpgradeStageDirectoryCreate');
  assert.equal(killed.signal, 'SIGKILL', `${killed.stdout}\n${killed.stderr}`);
  appendFileSync(join(target, UPGRADE_TRANSACTION_FILE),
    '{"api_version":"forgeos.scaffold-upgrade-stage-intent/v1"');
  run({ from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW);
  assert.deepEqual(readFileSync(join(target, CHANGED)), readFileSync(join(SOURCE_ROOT, CHANGED)));
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), false);
});

test('stage intent append limit rejects without changing journal bytes', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'forge-upgrade-intent-limit-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const journal = join(root, 'journal');
  writeFileSync(journal, Buffer.alloc(1024 * 1024), { mode: 0o600 });
  chmodSync(journal, 0o600);
  const opened = lstatSync(journal, { bigint: true });
  const transaction = {
    control: {
      identities: new Map([['journal', { dev: opened.dev, ino: opened.ino }]]),
      paths: { journal },
    },
    document: { entries: [{ stage_name: 'changed-entry' }] },
    hooks: {}, id: 'a'.repeat(32), stageIntents: new Map(),
  };
  const before = createHash('sha256').update(readFileSync(journal)).digest('hex');
  assert.throws(() => appendUpgradeStageIntent(transaction, {
    directoryIdentity: { dev: opened.dev, ino: opened.ino }, nextMode: 0o644,
    nextSha256: 'b'.repeat(64), stageName: 'changed-entry',
  }), /exceed journal limit/);
  assert.equal(lstatSync(journal).size, 1024 * 1024);
  assert.equal(createHash('sha256').update(readFileSync(journal)).digest('hex'), before);
});

test('an oversized whole plan is rejected before journal publication', (t) => {
  const { target } = project(t, 'journal-capacity-preflight');
  const added = Array.from({ length: 501 }, (_, index) => (
    `${String(index).padStart(4, '0')}-${'x'.repeat(1700)}`
  ));
  assert.throws(() => beginUpgradeTransaction({
    added, backups: [], changed: [], projectInstances: [], removed: [], targetDir: target,
    statePath: join(target, '.agent', '.forge-scaffold-state.json'),
  }), /plan exceeds journal intent capacity/);
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), false);
  assert.deepEqual(readdirSync(join(target, '.agent')).filter(
    (name) => name.includes('.forge-upgrade-transaction-v1.json'),
  ), []);
});

for (const hook of [
  'afterUpgradeStageIntentWrite', 'afterUpgradeStageNextSync',
  'afterUpgradePriorClaimSync',
]) {
  test(`SIGKILL at ${hook} is recovered from durable stage intent`, (t) => {
    const { target } = project(t, hook);
    forceChanged(target);
    const killed = crash(target, hook);
    assert.equal(killed.signal, 'SIGKILL', `${killed.stdout}\n${killed.stderr}`);
    const abandoned = stageFor(target, 'changed', CHANGED);
    const lines = readFileSync(join(target, UPGRADE_TRANSACTION_FILE), 'utf8').trimEnd().split('\n');
    assert.equal(lines.length, 2);
    run({ from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW);
    assert.deepEqual(readFileSync(join(target, CHANGED)), readFileSync(join(SOURCE_ROOT, CHANGED)));
    assert.equal(existsSync(abandoned), false);
    assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), false);
  });
}

test('prepared recovery preserves a changed target nonstandard mode', (t) => {
  const { target } = project(t, 'prepared-prior-mode');
  forceChanged(target);
  chmodSync(join(target, CHANGED), 0o640);
  const killed = crash(target, 'afterTransactionStart');
  assert.equal(killed.signal, 'SIGKILL', `${killed.stdout}\n${killed.stderr}`);
  run({ from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW);
  assert.deepEqual(readFileSync(join(target, CHANGED)), readFileSync(join(SOURCE_ROOT, CHANGED)));
  assert.equal(lstatSync(join(target, CHANGED)).mode & 0o777, 0o640);
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), false);
});

test('unstarted recovery preserves an unclaimed stage with unknown contents', (t) => {
  const { target } = project(t, 'unknown-preclaim-contents');
  forceChanged(target);
  const killed = crash(target, 'afterUpgradeStageDirectoryCreate');
  assert.equal(killed.signal, 'SIGKILL', `${killed.stdout}\n${killed.stderr}`);
  const abandoned = stageFor(target, 'changed', CHANGED);
  const unknown = join(abandoned, 'unknown');
  writeFileSync(unknown, 'concurrent owner\n');
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW,
  ), /preserved unclaimed upgrade stage artifacts/);
  assert.equal(readFileSync(unknown, 'utf8'), 'concurrent owner\n');
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), true);
});

test('unstarted recovery rejects a self-bound stage without durable intent', (t) => {
  const { target } = project(t, 'self-bound-without-intent');
  forceChanged(target);
  const killed = crash(target, 'afterUpgradeStageDirectoryCreate');
  assert.equal(killed.signal, 'SIGKILL', `${killed.stdout}\n${killed.stderr}`);
  const stage = stageFor(target, 'changed', CHANGED);
  const journal = join(target, UPGRADE_TRANSACTION_FILE);
  writeFileSync(journal, `${readFileSync(journal, 'utf8').split('\n')[0]}\n`);
  const entry = transaction(target).entries.find(
    (item) => item.kind === 'changed' && item.rel === CHANGED,
  );
  const bytes = Buffer.from('unknown self-bound next\n');
  const forged = forgeSelfBoundStage(stage, entry, bytes);
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW,
  ), /preserved unclaimed upgrade stage artifacts/);
  assert.deepEqual(readFileSync(forged.next), bytes);
  assert.equal(existsSync(forged.claim), true);
  assert.equal(existsSync(join(target, UPGRADE_TRANSACTION_FILE)), true);
});

test('no-intent recovery preserves a foreign empty stage replacement', (t) => {
  const { target } = project(t, 'foreign-empty-preclaim');
  forceChanged(target);
  const killed = crash(target, 'afterUpgradeStageDirectoryCreate');
  assert.equal(killed.signal, 'SIGKILL', `${killed.stdout}\n${killed.stderr}`);
  const stage = stageFor(target, 'changed', CHANGED);
  const original = `${stage}.original`;
  const journal = join(target, UPGRADE_TRANSACTION_FILE);
  writeFileSync(journal, `${readFileSync(journal, 'utf8').split('\n')[0]}\n`);
  renameSync(stage, original);
  mkdirSync(stage, { mode: 0o700 });
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW,
  ), /preserved unclaimed upgrade stage artifacts/);
  assert.deepEqual(readdirSync(stage), []);
  assert.equal(existsSync(original), true);
  assert.equal(existsSync(journal), true);
});

test('unstarted recovery restores an exact transaction-owned package closure', (t) => {
  const { target } = project(t, 'owned-package-preclaim-kill');
  const packagePath = join('harness', 'decision_capsule_contract');
  rmSync(join(target, packagePath), { recursive: true });
  const killed = crash(target, 'afterUpgradeStageDirectoryCreate', 'decision_capsule_contract');
  assert.equal(killed.signal, 'SIGKILL', `${killed.stdout}\n${killed.stderr}`);
  run({ from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW);
  const closure = (root) => readdirSync(join(root, packagePath), { recursive: true }).sort();
  assert.deepEqual(closure(target), closure(SOURCE_ROOT));
});

test('fixed prior-live collision is preserved and never overwritten', (t) => {
  const { target } = project(t, 'prior-live-collision');
  forceChanged(target);
  const unknown = Buffer.from('unknown prior-live owner\n');
  let collision;
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW,
    { beforePriorDetach({ destination, rel }) {
      if (rel !== CHANGED) return;
      collision = join(stageFor(target, 'changed', rel), 'prior-live');
      writeFileSync(collision, unknown);
      chmodSync(collision, 0o600);
      assert.equal(dirname(destination), dirname(join(target, rel)));
    } },
  ), /cleanup failed|ambiguous|not empty/i);
  assert.deepEqual(readFileSync(preservedStageArtifact(dirname(collision), 'prior-live')), unknown);
});

test('fixed rollback-live collision is preserved and never overwritten', (t) => {
  const { target } = project(t, 'rollback-live-collision');
  rmSync(join(target, ADDED));
  const unknown = Buffer.from('unknown rollback-live owner\n');
  let collision;
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW,
    {
      afterScaffoldStateWrite() { throw new Error('force rollback'); },
      beforeAddedRollbackRename({ rel }) {
        if (rel !== ADDED) return;
        collision = join(stageFor(target, 'added', rel), 'rollback-live');
        writeFileSync(collision, unknown);
        chmodSync(collision, 0o600);
      },
    },
  ), /rollback failed|force rollback/i);
  assert.deepEqual(readFileSync(preservedStageArtifact(dirname(collision), 'rollback-live')), unknown);
});

test('prepared authority rejects a self-consistent forged stage claim', (t) => {
  const { target } = project(t, 'forged-stage-claim');
  forceChanged(target);
  assert.equal(crash(target, 'afterTransactionStart').signal, 'SIGKILL');
  const stage = stageFor(target, 'changed', CHANGED);
  rmSync(stage, { recursive: true });
  mkdirSync(stage, { mode: 0o700 });
  const next = join(stage, 'next');
  const claim = join(stage, 'stage-claim.json');
  writeFileSync(next, 'forged next\n');
  chmodSync(next, 0o600);
  writeFileSync(claim, '');
  chmodSync(claim, 0o600);
  const directoryStat = lstatSync(stage, { bigint: true });
  const nextStat = lstatSync(next, { bigint: true });
  const claimStat = lstatSync(claim, { bigint: true });
  const entry = transaction(target).entries.find(
    (item) => item.kind === 'changed' && item.rel === CHANGED,
  );
  writeFileSync(claim, `${JSON.stringify({
    api_version: 'forgeos.scaffold-upgrade-stage-claim/v1',
    control: { dev: String(claimStat.dev), ino: String(claimStat.ino) },
    directory: { dev: String(directoryStat.dev), ino: String(directoryStat.ino) },
    next: {
      dev: String(nextStat.dev), ino: String(nextStat.ino), mode: 0o600,
      sha256: '0'.repeat(64),
    },
    stage_name: entry.stage_name,
  })}\n`);
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW,
  ), /changed (?:scaffold upgrade|recovery) stage authority/);
  assert.equal(existsSync(claim), true);
});

test('self-bound cleanup proof cannot authorize deletion without its journal', (t) => {
  const { target } = project(t, 'forged-cleanup-proof');
  const id = 'a'.repeat(32);
  const journal = join(target, UPGRADE_TRANSACTION_FILE);
  const finished = `${journal}.finished`;
  const proof = `${journal}.cleanup-proof`;
  writeFileSync(finished, `${id}\n`, { mode: 0o600 });
  writeFileSync(proof, '', { mode: 0o600 });
  const proofStat = lstatSync(proof, { bigint: true });
  const finishedStat = lstatSync(finished, { bigint: true });
  const rootStat = statSync(target, { bigint: true });
  const stored = (stat) => ({ dev: String(stat.dev), ino: String(stat.ino) });
  writeFileSync(proof, `${JSON.stringify({
    api_version: 'forgeos.scaffold-upgrade-control-cleanup/v1',
    controls: {
      committed: null, finished: stored(finishedStat), journal: stored(proofStat),
      prepared: null, started: null,
    },
    proof: stored(proofStat), target: stored(rootStat), transaction_id: id,
  })}\n`);
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW,
  ), /orphaned scaffold upgrade transaction marker/);
  assert.equal(existsSync(proof), true);
  assert.equal(existsSync(finished), true);
});

test('commit parent fsync is irreversible even when the target root swaps next', (t) => {
  const { root, target } = project(t, 'commit-linearization');
  const prior = forceChanged(target);
  const original = join(root, 'committed-project');
  const replacement = join(root, 'replacement-project');
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW,
    { afterTransactionControlParentSync({ name }) {
      if (name !== 'committed') return;
      renameSync(target, original);
      mkdirSync(target);
      writeFileSync(join(target, 'sentinel.txt'), 'replacement owner\n');
    } },
  ), /committed|changed.*directory/i);
  assert.notDeepEqual(readFileSync(join(original, CHANGED)), prior);
  assert.deepEqual(readFileSync(join(original, CHANGED)), readFileSync(join(SOURCE_ROOT, CHANGED)));
  renameSync(target, replacement);
  renameSync(original, target);
  run({ from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW);
  assert.equal(readFileSync(join(replacement, 'sentinel.txt'), 'utf8'), 'replacement owner\n');
});

function copySourceProjection(root) {
  const paths = new Set([...manifestProjection(SOURCE_ROOT), ...PROJECT_INSTANCE_FILES]);
  for (const rel of paths) {
    const target = join(root, rel);
    mkdirSync(dirname(target), { recursive: true });
    copyFileSync(join(SOURCE_ROOT, rel), target);
  }
}

test('frozen source bytes bind provenance, classification, staging and ledger', (t) => {
  const { root, target } = project(t, 'frozen-source');
  const source = join(root, 'source');
  mkdirSync(source);
  copySourceProjection(source);
  const git = (...args) => spawnSync('git', args, { cwd: source, encoding: 'utf8' });
  assert.equal(git('init', '-q').status, 0);
  assert.equal(git('add', '-f', '.').status, 0);
  assert.equal(git('-c', 'user.name=Forge Test', '-c', 'user.email=forge@example.invalid',
    'commit', '-qm', 'source fixture').status, 0);
  const sourcePath = join(source, ADDED);
  const frozen = Buffer.from('// frozen ignored source bytes\n');
  const later = Buffer.from('// later source replacement\n');
  writeFileSync(sourcePath, frozen);
  assert.equal(git('update-index', '--assume-unchanged', ADDED).status, 0);
  const logs = [];
  const priorLog = console.log;
  console.log = (...values) => logs.push(values.join(' '));
  try {
    run({ from: source, target, apply: true, backup: false, prune: false }, NOW,
      { afterApplyReport() { rmSync(sourcePath); writeFileSync(sourcePath, later); } });
  } finally { console.log = priorLog; }
  assert.match(logs[0], /dirty working tree based on HEAD/);
  assert.deepEqual(readFileSync(join(target, ADDED)), frozen);
  assert.deepEqual(readFileSync(sourcePath), later);
});
