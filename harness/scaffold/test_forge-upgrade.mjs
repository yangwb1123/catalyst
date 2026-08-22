// Tests for harness/scaffold/forge-upgrade.mjs (node:test, zero external deps).
// Run: node --test harness/scaffold/test_forge-upgrade.mjs
//
// forge-upgrade resyncs an already-scaffolded project's COPIED governance (the
// 70%) from a ForgeOS SOURCE repo. These tests mirror migrate_test's fail-safe
// discipline (DRY writes nothing; --apply is the only mutator) and, above all,
// PIN THE RED LINE: upgrade must NEVER touch project identity (the 30%).
//
// Fixture strategy: the SOURCE repo is THIS repo (SOURCE_ROOT). A project is built
// by spawning the real forge-init (the proven scaffold path) into a tmp dir, so it
// carries the genuine copied 70% + generated 30%. Upgrade is then driven in-process
// via the exported run() (no process.exit), and we assert on the bytes on disk.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  mkdtempSync,
  mkdirSync,
  rmSync,
  existsSync,
  readFileSync,
  writeFileSync,
  readdirSync,
  symlinkSync,
} from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';

import { run, classifyDrift, manifestProjection } from './forge-upgrade.mjs';
import { SCAFFOLD_STATE_FILE } from './forge-init.mjs';

// This test lives in harness/scaffold/, so the repo root (= the SOURCE repo used
// as the upgrade fixture) is TWO levels up; forge-init is its same-dir sibling.
const SCAFFOLD_DIR = dirname(fileURLToPath(import.meta.url));
const SOURCE_ROOT = dirname(dirname(SCAFFOLD_DIR));
const INIT_PATH = join(SCAFFOLD_DIR, 'forge-init.mjs');

// The project IDENTITY files upgrade must NEVER write — the 30%. Kept in lockstep
// with forge-init's writeGenerated() calls (and test_forge-init's GENERATED_FILES).
// Test #2 asserts upgrade's written set is DISJOINT from this list.
const GENERATED_FILES = [
  join('.agent', 'PROJECT.md'),
  join('.agent', 'ROADMAP.md'),
  join('.agent', 'CURRENT_SPRINT.md'),
  join('.agent', 'project.yml'),
  'CLAUDE.md',
  join('.github', 'workflows', 'forge.yml'),
  join('examples', 'starter', 'package.json'),
  join('examples', 'starter', 'src', 'greet.mjs'),
  join('examples', 'starter', 'test', 'greet.test.mjs'),
  'README.md',
  '.gitignore',
];

// Scaffold a fresh project into a tmp dir via the real forge-init; returns its path.
function scaffoldProject(t, name = 'upgrade-fixture') {
  const dir = mkdtempSync(join(tmpdir(), 'forge-upgrade-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const target = join(dir, 'proj');
  const res = spawnSync(process.execPath, [INIT_PATH, target, '--name', name], {
    encoding: 'utf8',
  });
  assert.equal(res.status, 0, `forge-init must scaffold the fixture; stderr:\n${res.stderr}`);
  return target;
}

// Snapshot every file under `dir` (recursively) as rel -> bytes (Buffer). Used to
// prove "not one byte changed" across a whole subtree.
function snapshotTree(dir, base = dir, acc = new Map()) {
  for (const ent of readdirSync(dir, { withFileTypes: true })) {
    const abs = join(dir, ent.name);
    if (ent.isDirectory()) snapshotTree(abs, base, acc);
    else acc.set(abs.slice(base.length + 1), readFileSync(abs));
  }
  return acc;
}

// pyYamlAvailable: does python3 (resolved from cwd) have PyYAML? acceptance's
// check.py exits 2 without it — an ENV prereq (CI installs it), so test #6's
// ACCEPTED assertion is skipped-with-reason when absent, mirroring test_forge-init.
function pyYamlAvailable(cwd) {
  const r = spawnSync('python3', ['-c', 'import yaml'], { cwd, encoding: 'utf8' });
  return r.status === 0;
}

// Most-recent child dir name under a parent (the backup timestamp dir).
function onlyChildDir(parent) {
  const kids = readdirSync(parent, { withFileTypes: true }).filter((e) => e.isDirectory());
  assert.equal(kids.length, 1, `expected exactly one backup dir under ${parent}, got ${kids.length}`);
  return join(parent, kids[0].name);
}

function projectedSource(t) {
  const dir = mkdtempSync(join(tmpdir(), 'forge-upgrade-source-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const source = join(dir, 'source');
  for (const rel of manifestProjection(SOURCE_ROOT)) {
    const destination = join(source, rel);
    mkdirSync(dirname(destination), { recursive: true });
    writeFileSync(destination, readFileSync(join(SOURCE_ROOT, rel)));
  }
  return source;
}

// ① DRY writes NOTHING. Mutate the project's copied acceptance.mjs and DELETE its
// copied sca.mjs, then run upgrade WITHOUT --apply. Every project byte must be
// unchanged, and the report must name the drift (changed acceptance / added sca).
test('dry run writes nothing and reports the drift', (t) => {
  const target = scaffoldProject(t);
  const accRel = join('harness', 'acceptance.mjs');
  const scaRel = join('harness', 'sca.mjs');

  // Simulate lag: project's acceptance.mjs differs from source; sca.mjs missing.
  writeFileSync(join(target, accRel), '// stale local copy — drifted\n');
  rmSync(join(target, scaRel));

  const before = snapshotTree(target);
  const res = run({ from: SOURCE_ROOT, target, apply: false, backup: true, prune: false });
  const after = snapshotTree(target);

  // not one byte written anywhere in the project (and no .forge backup dir made).
  assert.deepEqual([...after.keys()].sort(), [...before.keys()].sort(), 'dry run changed the file SET');
  for (const [rel, bytes] of before) {
    assert.ok(after.get(rel).equals(bytes), `dry run mutated ${rel}`);
  }
  assert.equal(existsSync(join(target, '.forge')), false, 'dry run created a backup dir');

  // the report classifies the drift correctly.
  assert.ok(res.drift.changed.includes(accRel), 'acceptance.mjs should be reported changed');
  assert.ok(res.drift.added.includes(scaRel), 'sca.mjs should be reported added');
  assert.equal(res.applied, false);
});

// ② ★ THE RED LINE (highest priority): upgrade NEVER touches identity. ★
// Stamp a UNIQUE marker into EVERY generated identity file, then run --apply
// (which resyncs the 70%). Each identity file must be byte-for-byte unchanged —
// the marker still present — proving upgrade's write set is disjoint from the 30%.
test('apply never touches project identity (the 30%)', (t) => {
  const target = scaffoldProject(t);

  // Stamp each identity file with a unique sentinel and remember its exact bytes.
  const stamped = new Map();
  for (const rel of GENERATED_FILES) {
    const p = join(target, rel);
    const marker = `\n# FORGE-UPGRADE-MUST-NOT-TOUCH ${rel} ${Math.random()}\n`;
    const bytes = Buffer.concat([readFileSync(p), Buffer.from(marker)]);
    writeFileSync(p, bytes);
    stamped.set(rel, bytes);
  }
  // Also create REAL drift in the 70% so --apply genuinely has work to do.
  writeFileSync(join(target, 'harness', 'gate.mjs'), '// drifted\n');

  const res = run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false });

  // EVERY identity file is byte-identical to its stamped version (untouched).
  for (const rel of GENERATED_FILES) {
    assert.ok(
      readFileSync(join(target, rel)).equals(stamped.get(rel)),
      `RED LINE VIOLATED: upgrade modified identity file ${rel}`,
    );
  }
  // And structurally: the written set is DISJOINT from the identity set.
  const writtenSet = new Set([...res.drift.changed, ...res.drift.added]);
  for (const rel of GENERATED_FILES) {
    assert.equal(writtenSet.has(rel), false, `written set must not contain identity file ${rel}`);
  }
  // gate.mjs (a 70% file) WAS resynced — proving apply did real work, not a no-op.
  assert.ok(res.drift.changed.includes(join('harness', 'gate.mjs')), 'gate.mjs should have been resynced');
});

// ③ apply SYNCS: after --apply, every manifest-projection file is byte-identical
// to its SOURCE counterpart (the whole copied 70% is back in lockstep).
test('apply resyncs the copied 70% to byte-identical-with-source', (t) => {
  const target = scaffoldProject(t);
  // Drift two files: one changed, one deleted (added back on apply).
  writeFileSync(join(target, 'harness', 'acceptance.mjs'), '// stale\n');
  rmSync(join(target, 'harness', 'sca.mjs'));

  run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false });

  for (const rel of manifestProjection(SOURCE_ROOT)) {
    assert.ok(
      readFileSync(join(target, rel)).equals(readFileSync(join(SOURCE_ROOT, rel))),
      `after apply, ${rel} must equal source`,
    );
  }
  // post-apply, a fresh classification finds zero drift.
  const drift = classifyDrift(SOURCE_ROOT, target);
  assert.deepEqual(drift.changed, [], 'no changed files remain after apply');
  assert.deepEqual(drift.added, [], 'no added files remain after apply');
});

// ④ BACKUP FIDELITY: a user-modified copied file is backed up VERBATIM before being
// overwritten — zero loss. After apply: the backup holds the USER's bytes, the
// project holds SOURCE's bytes.
test('apply backs up an overwritten file verbatim (zero loss)', (t) => {
  const target = scaffoldProject(t);
  const accRel = join('harness', 'acceptance.mjs');
  const userBytes = Buffer.from('// user-local edit to acceptance.mjs — must be backed up\n');
  writeFileSync(join(target, accRel), userBytes);

  const res = run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false });
  assert.ok(res.backedUp >= 1, 'at least one file should have been backed up');

  // project copy now equals SOURCE.
  assert.ok(
    readFileSync(join(target, accRel)).equals(readFileSync(join(SOURCE_ROOT, accRel))),
    'project acceptance.mjs should now equal source',
  );
  // the backup holds the USER's version, byte-for-byte.
  const backupRoot = join(target, '.forge', 'upgrade-backup');
  const ts = onlyChildDir(backupRoot);
  assert.ok(
    readFileSync(join(ts, accRel)).equals(userBytes),
    'backup must preserve the user version verbatim',
  );
});

// ⑤ IDEMPOTENT: --apply twice. The second run finds everything unchanged — zero
// writes, zero backups (no second backup dir created).
test('apply is idempotent: a second apply writes and backs up nothing', (t) => {
  const target = scaffoldProject(t);
  writeFileSync(join(target, 'harness', 'gate.mjs'), '// drifted\n');

  const first = run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false });
  assert.ok(first.written >= 1, 'first apply should write the drifted file');

  const second = run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false });
  assert.equal(second.written, 0, 'second apply must write nothing');
  assert.equal(second.backedUp, 0, 'second apply must back up nothing');

  // exactly ONE backup dir exists (the first run's); the second made none.
  const backupRoot = join(target, '.forge', 'upgrade-backup');
  const dirs = readdirSync(backupRoot, { withFileTypes: true }).filter((e) => e.isDirectory());
  assert.equal(dirs.length, 1, 'second apply must not create a second backup dir');
});

// ⑥ ACCEPTED after upgrade: a project whose 70% was resynced still passes the full
// acceptance gate (exit 0). Reuses test_forge-init's PyYAML skip (an ENV prereq).
test('project is ACCEPTED after an apply', {
  skip: Boolean(process.env.FORGE_ACCEPT_INNER),
}, (t) => {
  const target = scaffoldProject(t);
  writeFileSync(join(target, 'harness', 'acceptance.mjs'), '// stale, will be resynced\n');
  run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false });

  if (!pyYamlAvailable(target)) {
    t.skip('PyYAML unavailable — ACCEPTED assertion skipped (env prereq; CI installs it)');
    return;
  }
  const acc = spawnSync(process.execPath, ['harness/acceptance.mjs'], { cwd: target, encoding: 'utf8' });
  assert.equal(
    acc.status, 0,
    `upgraded project must be ACCEPTED; exit ${acc.status}\nstdout:\n${acc.stdout}\nstderr:\n${acc.stderr}`,
  );
  assert.match(acc.stdout, /forge-accept: ACCEPTED/);
});

// ⑦ --no-backup honored: with --no-backup, an overwrite takes NO backup (no .forge
// dir), proving the opt-out path. (The default-on backup is covered by #4.)
test('--no-backup skips the backup entirely', (t) => {
  const target = scaffoldProject(t);
  writeFileSync(join(target, 'harness', 'gate.mjs'), '// drifted\n');

  const res = run({ from: SOURCE_ROOT, target, apply: true, backup: false, prune: false });
  assert.ok(res.written >= 1, 'apply should still write with --no-backup');
  assert.equal(res.backedUp, 0, '--no-backup must take no backups');
  assert.equal(existsSync(join(target, '.forge')), false, '--no-backup must not create .forge');
});

// ⑧ PRUNE is real but ledger-constrained: a path recorded by the prior scaffold
// and retired from the current source is backed up + deleted. An unrecorded user
// file in the same harness directory is never inferred as removable.
test('--prune backs up and removes only ledger-recorded retired files', (t) => {
  const target = scaffoldProject(t);
  const retired = join('harness', 'retired-tool.mjs');
  const userFile = join('harness', 'user-local.mjs');
  const retiredBytes = Buffer.from('// retired copied tool\n');
  writeFileSync(join(target, retired), retiredBytes);
  writeFileSync(join(target, userFile), '// user-owned extension\n');

  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  state.copied.push(retired);
  writeFileSync(statePath, `${JSON.stringify(state, null, 2)}\n`);

  const dry = run({ from: SOURCE_ROOT, target, apply: false, backup: true, prune: true });
  assert.ok(dry.removed.includes(retired), 'dry report must identify the retired ledger path');
  assert.equal(existsSync(join(target, retired)), true, 'dry --prune must not delete');

  const res = run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: true });
  assert.equal(res.pruned, 1);
  assert.equal(existsSync(join(target, retired)), false, 'retired copied file must be deleted');
  assert.equal(existsSync(join(target, userFile)), true, 'unrecorded user file must survive');
  const backup = onlyChildDir(join(target, '.forge', 'upgrade-backup'));
  assert.ok(readFileSync(join(backup, retired)).equals(retiredBytes), 'pruned bytes must be recoverable');
  const updated = JSON.parse(readFileSync(statePath, 'utf8'));
  assert.equal(updated.copied.includes(retired), false, 'pruned path leaves the persisted projection');
});

test('--prune rejects a tampered ledger path outside governance namespaces', (t) => {
  const target = scaffoldProject(t);
  const readme = readFileSync(join(target, 'README.md'));
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  state.copied.push('README.md');
  writeFileSync(statePath, `${JSON.stringify(state, null, 2)}\n`);

  assert.throws(
    () => run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: true }),
    /unsafe path.*README\.md/,
  );
  assert.ok(readFileSync(join(target, 'README.md')).equals(readme), 'identity file must remain untouched');
});

test('--prune permits only the fixed release contract path, not arbitrary docs files', (t) => {
  const target = scaffoldProject(t);
  const unsafe = join('docs', 'product-notes.md');
  mkdirSync(dirname(join(target, unsafe)), { recursive: true });
  writeFileSync(join(target, unsafe), 'user-owned\n');
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  assert.ok(state.copied.includes(join('docs', 'release', 'README.md')));
  state.copied.push(unsafe);
  writeFileSync(statePath, `${JSON.stringify(state, null, 2)}\n`);

  assert.throws(
    () => run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: true }),
    /unsafe path.*docs.*product-notes\.md/,
  );
  assert.equal(readFileSync(join(target, unsafe), 'utf8'), 'user-owned\n');
});

test('apply refuses to overwrite a symlinked copied file', (t) => {
  const target = scaffoldProject(t);
  const gate = join(target, 'harness', 'gate.mjs');
  const outside = join(dirname(target), 'outside-gate.mjs');
  const outsideBytes = Buffer.from('// outside user file\n');
  writeFileSync(outside, outsideBytes);
  rmSync(gate);
  symlinkSync(outside, gate);

  assert.throws(
    () => run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false }),
    /unsafe symlink path.*gate\.mjs/,
  );
  assert.ok(readFileSync(outside).equals(outsideBytes), 'external symlink target must not change');
});

test('upgrade refuses a symlink target and a symlinked harness ancestor', (t) => {
  const target = scaffoldProject(t);
  const parent = dirname(target);
  const outsideTarget = join(parent, 'outside-target');
  mkdirSync(outsideTarget);
  writeFileSync(join(outsideTarget, 'sentinel'), 'external\n');
  rmSync(target, { recursive: true, force: true });
  symlinkSync(outsideTarget, target);
  assert.throws(
    () => run({ from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }),
    /unsafe symlink path.*target directory/i,
  );
  assert.deepEqual(readdirSync(outsideTarget), ['sentinel']);

  rmSync(target);
  mkdirSync(target);
  const outsideHarness = join(parent, 'outside-harness');
  mkdirSync(outsideHarness);
  symlinkSync(outsideHarness, join(target, 'harness'));
  assert.throws(
    () => run({ from: SOURCE_ROOT, target, apply: true, backup: false, prune: false }),
    /unsafe symlink path.*harness/i,
  );
  assert.equal(readdirSync(outsideHarness).length, 0, 'apply must not write through harness link');
});

test('upgrade refuses a symlinked parent above the target directory', (t) => {
  const target = scaffoldProject(t);
  const parent = dirname(target);
  const linkedParent = join(parent, 'linked-parent');
  symlinkSync(parent, linkedParent);

  assert.throws(
    () => run({
      from: SOURCE_ROOT,
      target: join(linkedParent, 'proj'),
      apply: true,
      backup: false,
      prune: false,
    }),
    /unsafe symlink path.*target directory/i,
  );
  assert.equal(existsSync(join(target, '.forge')), false, 'linked-parent rejection must write nothing');
});

test('upgrade refuses a symlinked scaffold-state.json before reading it', (t) => {
  const target = scaffoldProject(t);
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const outsideState = join(dirname(target), 'outside-state.json');
  const outsideBytes = Buffer.from('{"copied":["harness/gate.mjs"]}\n');
  writeFileSync(outsideState, outsideBytes);
  rmSync(statePath);
  symlinkSync(outsideState, statePath);

  assert.throws(
    () => run({ from: SOURCE_ROOT, target, apply: false, backup: true, prune: true }),
    /unsafe symlink path.*scaffold-state\.json/i,
  );
  assert.ok(readFileSync(outsideState).equals(outsideBytes), 'external state must not be read through or changed');
});

test('upgrade rejects symlinks in the governed --from projection', (t) => {
  const target = scaffoldProject(t);
  const source = projectedSource(t);
  const sourceGate = join(source, 'harness', 'gate.mjs');
  const outsideSource = join(dirname(source), 'outside-source.mjs');
  writeFileSync(outsideSource, '// outside source bytes\n');
  rmSync(sourceGate);
  symlinkSync(outsideSource, sourceGate);

  assert.throws(
    () => run({ from: source, target, apply: false, backup: true, prune: false }),
    /unsafe symlink path.*source.*gate\.mjs/i,
  );
  assert.equal(existsSync(join(target, '.forge')), false, 'source rejection must happen before target writes');
});

test('upgrade refuses symlinks at every backup ancestor including timestamp', (t) => {
  const now = new Date('2026-07-27T12:00:00.000Z');
  const timestamp = '2026-07-27T12-00-00.000Z';
  const cases = [
    {
      name: '.forge',
      setup(target, outside) {
        symlinkSync(outside, join(target, '.forge'));
      },
    },
    {
      name: 'upgrade-backup',
      setup(target, outside) {
        mkdirSync(join(target, '.forge'));
        symlinkSync(outside, join(target, '.forge', 'upgrade-backup'));
      },
    },
    {
      name: 'timestamp',
      setup(target, outside) {
        mkdirSync(join(target, '.forge', 'upgrade-backup'), { recursive: true });
        symlinkSync(outside, join(target, '.forge', 'upgrade-backup', timestamp));
      },
    },
  ];

  for (const fixture of cases) {
    const target = scaffoldProject(t, `backup-link-${fixture.name}`);
    const outside = join(dirname(target), `outside-${fixture.name}`);
    mkdirSync(outside);
    writeFileSync(join(target, 'harness', 'gate.mjs'), '// drift requiring backup\n');
    fixture.setup(target, outside);

    assert.throws(
      () => run(
        { from: SOURCE_ROOT, target, apply: true, backup: true, prune: false },
        now,
      ),
      /unsafe symlink path.*backup/i,
      `${fixture.name} symlink must be rejected`,
    );
    assert.equal(readdirSync(outside).length, 0, `${fixture.name} link must receive no backup`);
    assert.equal(
      readFileSync(join(target, 'harness', 'gate.mjs'), 'utf8'),
      '// drift requiring backup\n',
      `${fixture.name} rejection must precede governed overwrites`,
    );
  }
});

test('--prune refuses a symlinked ancestor before deleting an external retired file', (t) => {
  const target = scaffoldProject(t);
  const retired = join('harness', 'retired-tool.mjs');
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  state.copied.push(retired);
  writeFileSync(statePath, `${JSON.stringify(state, null, 2)}\n`);

  const outsideHarness = join(dirname(target), 'outside-prune-harness');
  mkdirSync(outsideHarness);
  const outsideRetired = join(outsideHarness, 'retired-tool.mjs');
  writeFileSync(outsideRetired, '// must survive\n');
  rmSync(join(target, 'harness'), { recursive: true, force: true });
  symlinkSync(outsideHarness, join(target, 'harness'));

  assert.throws(
    () => run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: true }),
    /unsafe symlink path.*harness/i,
  );
  assert.equal(readFileSync(outsideRetired, 'utf8'), '// must survive\n');
  assert.equal(existsSync(join(target, '.forge')), false, 'rejection must precede backup writes');
});
