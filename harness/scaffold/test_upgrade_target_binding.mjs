import test from 'node:test';
import assert from 'node:assert/strict';
import {
  copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync,
  renameSync, rmSync, writeFileSync,
} from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import {
  PROJECT_INSTANCE_FILES, renderScaffoldState, scaffold, SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';
import { manifestProjection, run } from './forge-upgrade.mjs';

const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const UPGRADE_URL = pathToFileURL(join(SOURCE_ROOT, 'harness', 'scaffold', 'forge-upgrade.mjs')).href;
const NOW = new Date('2026-08-21T15:00:00.000Z');

function copySourceProjection(destination) {
  for (const rel of new Set([...manifestProjection(SOURCE_ROOT), ...PROJECT_INSTANCE_FILES])) {
    const path = join(destination, rel);
    mkdirSync(dirname(path), { recursive: true });
    copyFileSync(join(SOURCE_ROOT, rel), path);
  }
}

test('source capture rejects a root replacement without publishing a mixed plan', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'forge-upgrade-source-binding-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const source = join(root, 'source');
  const replacement = join(root, 'replacement');
  const parked = join(root, 'captured-source');
  scaffold({ target, name: 'source-binding', mode: 'balanced', lifecycle: 'mvp', force: false });
  mkdirSync(source); mkdirSync(replacement); copySourceProjection(source);
  const governed = join(target, 'harness', 'gate.mjs');
  const prior = Buffer.from('// target must remain untouched\n');
  writeFileSync(governed, prior);

  assert.throws(() => run(
    { from: source, target, apply: true, backup: false, prune: false }, NOW,
    { afterSourceSnapshotRead({ index }) {
      if (index !== 0) return;
      renameSync(source, parked); renameSync(replacement, source);
    } },
  ), /changed directory boundary.*source session/i);
  assert.deepEqual(readFileSync(governed), prior);
  assert.equal(existsSync(join(target, '.agent', '.forge-upgrade-transaction-v1.json')), false);
});

test('an unchanged target mutated after its classification is never adopted', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'forge-upgrade-classification-binding-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  scaffold({ target, name: 'classification-binding', mode: 'balanced', lifecycle: 'mvp', force: false });
  const replacement = Buffer.from('// post-classification owner bytes\n');
  let changed;
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW,
    { afterTargetClassificationRead({ index, rel }) {
      if (index !== 0) return;
      changed = join(target, rel); writeFileSync(changed, replacement);
    } },
  ), /destination changed after classification/i);
  assert.deepEqual(readFileSync(changed), replacement);
});

test('a changed target mutated after classification is preserved without residue', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'forge-upgrade-changed-binding-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  scaffold({ target, name: 'changed-binding', mode: 'balanced', lifecycle: 'mvp', force: false });
  const rel = join('harness', 'gate.mjs');
  const destination = join(target, rel);
  const replacement = Buffer.from('// concurrent changed owner bytes\n');
  writeFileSync(destination, '// classified changed bytes\n');
  assert.throws(() => run(
    { from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }, NOW,
    { afterTargetClassificationRead({ rel: observed }) {
      if (observed === rel) writeFileSync(destination, replacement);
    } },
  ), /destination changed after classification/i);
  assert.deepEqual(readFileSync(destination), replacement);
  assert.equal(existsSync(join(target, '.agent', '.forge-upgrade-transaction-v1.json')), false);
});

test('classification remains bound to its retained target inode during a transient root swap', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'forge-upgrade-target-binding-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const replacement = join(root, 'replacement');
  const parked = join(root, 'classified-project');
  scaffold({ target, name: 'classified', mode: 'balanced', lifecycle: 'mvp', force: false });
  scaffold({ target: replacement, name: 'replacement', mode: 'balanced', lifecycle: 'mvp', force: false });

  const projection = manifestProjection(SOURCE_ROOT);
  const rel = projection[Math.floor(projection.length / 2)];
  const replacementState = join(replacement, SCAFFOLD_STATE_FILE);
  const copied = JSON.parse(readFileSync(replacementState, 'utf8')).copied
    .filter((item) => item !== rel);
  const unknown = Buffer.from('replacement root owns these bytes\n');
  writeFileSync(join(target, rel), '// classified target drift\n');
  writeFileSync(replacementState, renderScaffoldState(copied));
  writeFileSync(join(replacement, rel), unknown);

  const result = run(
    { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW,
    { afterTargetClassificationRead({ index, total }) {
      if (index === 0) {
        renameSync(target, parked);
        renameSync(replacement, target);
      } else if (index === total - 1) {
        renameSync(target, replacement);
        renameSync(parked, target);
      }
    } },
  );

  assert.equal(result.applied, true);
  assert.deepEqual(readFileSync(join(target, rel)), readFileSync(join(SOURCE_ROOT, rel)));
  assert.deepEqual(readFileSync(join(replacement, rel)), unknown);
});

function transactionArtifacts(root, found = []) {
  if (!existsSync(root)) return found;
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.name.startsWith('.forge-upgrade-')) found.push(path);
    if (entry.isDirectory()) transactionArtifacts(path, found);
  }
  return found;
}

test('a crash after stage-directory detachment resumes from its external inode proof', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'forge-upgrade-stage-detach-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  scaffold({ target, name: 'stage-detach', mode: 'balanced', lifecycle: 'mvp', force: false });
  const rel = join('.agent', 'AGENTS.md');
  writeFileSync(join(target, rel), '// force stage publication\n');
  const child = `
    import { run } from ${JSON.stringify(UPGRADE_URL)};
    const [source, target, hook] = process.argv.slice(1);
    run({ from: source, target, apply: true, backup: true, prune: false },
      new Date(${JSON.stringify(NOW.toISOString())}),
      { [hook]() { process.kill(process.pid, 'SIGKILL'); } });
  `;
  const crashed = spawnSync(
    process.execPath,
    ['--input-type=module', '-e', child, SOURCE_ROOT, target,
      'afterUpgradeStageDirectoryDetach'],
    { encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 },
  );
  assert.equal(crashed.signal, 'SIGKILL', `${crashed.stdout}\n${crashed.stderr}`);
  assert.ok(transactionArtifacts(target).some((path) => path.includes('cleanup-proof')));

  run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW);
  assert.deepEqual(readFileSync(join(target, rel)), readFileSync(join(SOURCE_ROOT, rel)));
  assert.deepEqual(transactionArtifacts(target), []);
});

test('a crash after control detachment resumes from its self-bound cleanup proof', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'forge-upgrade-control-detach-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  scaffold({ target, name: 'control-detach', mode: 'balanced', lifecycle: 'mvp', force: false });
  const rel = join('.agent', 'AGENTS.md');
  writeFileSync(join(target, rel), '// force control cleanup\n');
  const child = `
    import { run } from ${JSON.stringify(UPGRADE_URL)};
    const [source, target] = process.argv.slice(1);
    run({ from: source, target, apply: true, backup: true, prune: false },
      new Date(${JSON.stringify(NOW.toISOString())}), {
        afterTransactionControlDetach({ name }) {
          if (name === 'cleanup') process.kill(process.pid, 'SIGKILL');
        },
      });
  `;
  const crashed = spawnSync(
    process.execPath, ['--input-type=module', '-e', child, SOURCE_ROOT, target],
    { encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 },
  );
  assert.equal(crashed.signal, 'SIGKILL', `${crashed.stdout}\n${crashed.stderr}`);
  const interrupted = transactionArtifacts(target);
  assert.ok(interrupted.some((path) => path.includes('cleanup-proof')));
  assert.ok(interrupted.some((path) => path.includes('.removing-')));

  run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }, NOW);
  assert.deepEqual(readFileSync(join(target, rel)), readFileSync(join(SOURCE_ROOT, rel)));
  assert.deepEqual(transactionArtifacts(target), []);
});
