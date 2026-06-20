// Tests for harness/forge-init.mjs (node:test, zero external deps).
// Run: node --test harness/test_forge-init.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, existsSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';

import { COPIED_FILES, GOVERNANCE_DIRS, HARNESS_NOT_COPIED } from './forge-init.mjs';

const HARNESS_DIR = dirname(fileURLToPath(import.meta.url));
const SOURCE_ROOT = dirname(HARNESS_DIR);
const INIT_PATH = join(HARNESS_DIR, 'forge-init.mjs');

// Run forge-init as a child process; returns spawnSync result.
function runInit(args) {
  return spawnSync(process.execPath, [INIT_PATH, ...args], { encoding: 'utf8' });
}

// pyYamlAvailable: does the `python3` that the generated project's acceptance
// would invoke (resolved from `cwd`) have PyYAML? check.py exits 2 without it,
// failing arch_violations + test_pass — an ENVIRONMENT prerequisite (the CI
// installs it), not a scaffold defect. We probe so the ACCEPTED assertion is
// deterministic everywhere: skipped-with-reason when PyYAML is absent, asserted
// when present. Either way the copy/structure assertions always run.
function pyYamlAvailable(cwd) {
  const r = spawnSync('python3', ['-c', 'import yaml'], { cwd, encoding: 'utf8' });
  return r.status === 0;
}

// --- the universal governance a fresh project must INHERIT verbatim ---------

// (a) The host-independent ENFORCERS — each resolves paths from its own location.
const COPIED_ENFORCERS = [
  join('.agent', 'AGENTS.md'),
  join('harness', 'gate.mjs'),
  join('harness', 'policies.yml'),
  join('harness', 'arch', 'arch-check.mjs'),
  join('harness', 'arch', 'scan.mjs'),
  join('harness', 'arch', 'scan-functions.mjs'),
  join('harness', 'secret-scan.mjs'),
  join('.arch', 'rules.yaml'),
];

// (b) The rest of the FULL harness — check + accept + their inputs + EVERY
// self-test (acceptance's test_pass runs these, so the harness self-governs).
const COPIED_HARNESS = [
  join('harness', 'check.py'),
  join('harness', 'acceptance.mjs'),
  // adapters.mjs (imported by acceptance.mjs's lint criterion) + the per-language
  // command maps it reads at runtime + its self-test — all inherited verbatim so
  // the fresh project's acceptance gate imports and self-governs the module too.
  join('harness', 'adapters.mjs'),
  join('harness', 'adapters', 'go.yml'),
  join('harness', 'adapters', 'python.yml'),
  join('harness', 'adapters', 'typescript.yml'),
  join('harness', 'yaml2json.py'),
  join('harness', 'scorecard.mjs'),
  join('harness', 'scorecard-update.mjs'),
  join('harness', 'test_check.py'),
  join('harness', 'test_yaml2json.py'),
  join('harness', 'test_acceptance.mjs'),
  join('harness', 'test_adapters.mjs'),
  join('harness', 'test_gate.mjs'),
  join('harness', 'test_scorecard.mjs'),
  join('harness', 'test_scorecard-update.mjs'),
  join('harness', 'test_secret-scan.mjs'),
  join('harness', 'arch', 'test_arch-check.mjs'),
];

// (c) Representative governance ASSETS from each copied .agent/ tree — the
// declarative cards/skills/workflows/eval/routing/policies check.py validates
// and acceptance.mjs consumes (without them check.py FAILs / has no schema).
const COPIED_ASSETS = [
  join('.agent', 'agents', 'architect.md'),
  join('.agent', 'skills', 'clean-architecture.md'),
  join('.agent', 'workflows', 'build.yml'),
  join('.agent', 'eval', 'acceptance.schema.yml'),
  join('.agent', 'routing', 'policy.yml'),
  join('.agent', 'policies', 'modes.yml'),
];

// Everything copied VERBATIM must be byte-identical to its source.
const COPIED_VERBATIM = [...COPIED_ENFORCERS, ...COPIED_HARNESS, ...COPIED_ASSETS];

// GENERATED files (project identity + CC adapter + CI + seed app) — present, not
// byte-equal to any source.
const GENERATED_FILES = [
  join('.agent', 'PROJECT.md'),
  join('.agent', 'ROADMAP.md'),
  join('.agent', 'CURRENT_SPRINT.md'),
  join('.agent', 'project.yml'),
  'CLAUDE.md',
  join('.github', 'workflows', 'forge.yml'),
  join('examples', 'starter', 'src', 'greet.mjs'),
  join('examples', 'starter', 'test', 'greet.test.mjs'),
  'README.md',
  '.gitignore',
];

test('forge-init scaffolds COMPLETE governance and the project is ACCEPTED', (t) => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-init-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const target = join(dir, 'proj');

  const res = runInit([target, '--name', 'acme-svc']);
  assert.equal(res.status, 0, `init exit 0 expected; stderr:\n${res.stderr}`);

  // (1) every copied + generated file exists.
  for (const rel of [...COPIED_VERBATIM, ...GENERATED_FILES]) {
    assert.ok(existsSync(join(target, rel)), `missing scaffolded file: ${rel}`);
  }

  // (2) project identity carries the --name.
  assert.match(readFileSync(join(target, '.agent', 'project.yml'), 'utf8'), /acme-svc/);
  assert.match(readFileSync(join(target, '.agent', 'PROJECT.md'), 'utf8'), /acme-svc/);
  assert.match(readFileSync(join(target, 'CLAUDE.md'), 'utf8'), /acme-svc/);

  // (3) the generated CI gate runs `forge accept` (acceptance.mjs).
  assert.match(
    readFileSync(join(target, '.github', 'workflows', 'forge.yml'), 'utf8'),
    /node harness\/acceptance\.mjs/,
  );

  // (4) every verbatim-copied file (enforcers + full harness + assets) is
  // byte-identical to its source — the fresh project inherits the REAL tools and
  // governance, not a fork.
  for (const rel of COPIED_VERBATIM) {
    assert.deepEqual(
      readFileSync(join(target, rel)),
      readFileSync(join(SOURCE_ROOT, rel)),
      `copied ${rel} must be byte-identical to source`,
    );
  }

  // (5) ★ THE IRON PROOF: running the FULL acceptance gate on the fresh project
  // returns ACCEPTED — the complete governance (not just the enforcer triad) runs
  // end to end. PyYAML (check.py's only dep) is an ENVIRONMENT prerequisite the CI
  // installs; if it is absent we skip THIS assertion with a reason (the verdict
  // would falsely REJECT on check.py's honest exit-2), keeping the test
  // deterministic. The structure assertions above always ran regardless.
  if (!pyYamlAvailable(target)) {
    t.skip('PyYAML unavailable to python3 — acceptance ACCEPTED assertion skipped (env prereq; CI installs it)');
    return;
  }
  const acc = spawnSync(process.execPath, ['harness/acceptance.mjs'], { cwd: target, encoding: 'utf8' });
  assert.equal(
    acc.status, 0,
    `fresh project must be ACCEPTED; exit ${acc.status}\nstdout:\n${acc.stdout}\nstderr:\n${acc.stderr}`,
  );
  assert.match(acc.stdout, /forge-accept: ACCEPTED/);
  // The six load-bearing criteria are REAL PASSes (not faked, not N/A).
  for (const crit of [
    'test_pass', 'app_test_pass', 'complexity_violations',
    'arch_violations', 'architecture', 'security_findings',
  ]) {
    assert.match(acc.stdout, new RegExp(`\\[PASS\\] ${crit}`), `${crit} must PASS in the fresh project`);
  }
  // HONESTY: criteria with no tool stay visible as N/A (never silently dropped).
  assert.match(acc.stdout, /\[N-A \] coverage/);
  assert.match(acc.stdout, /\[N-A \] build/);
});

test('forge-init refuses to clobber a non-empty .agent without --force; --force succeeds', (t) => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-init-force-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const target = join(dir, 'proj');

  const first = runInit([target, '--name', 'acme-svc']);
  assert.equal(first.status, 0, `first init exit 0; stderr:\n${first.stderr}`);

  // re-init without --force is refused (non-zero) when .agent exists.
  const reinit = runInit([target, '--name', 'acme-svc']);
  assert.notEqual(reinit.status, 0, 're-init without --force must fail');
  assert.match(reinit.stderr, /--force/);

  // ...and --force succeeds.
  const forced = runInit([target, '--name', 'acme-svc', '--force']);
  assert.equal(forced.status, 0, `--force re-init must succeed; stderr:\n${forced.stderr}`);
});

// --- MANIFEST-INTEGRITY guard: COPIED_FILES must not drift from harness/ ------
// The blind spot this closes: COPIED_FILES is a HAND-MAINTAINED list with no
// guard that it stays in sync as harness/ grows. It already drifted — the real
// test_enforce.mjs (which pins the warn|block enforce middle, a module that IS
// copied) was dropped, so every scaffold silently ran less coverage. This walks
// harness/ and asserts EVERY source file is either copied, covered by a copied
// GOVERNANCE_DIR tree, or on the explicit HARNESS_NOT_COPIED whitelist — one
// guard that both fixes the drift and prevents the next regression.

// Recursively collect harness/ source files (.mjs/.py/.yml), returned as paths
// RELATIVE to SOURCE_ROOT (e.g. "harness/arch/scan.mjs") so they line up with
// the manifest's join('harness', ...) entries. Skips __pycache__ and READMEs
// (human-only prose, intentionally never copied — matches copyTree's skip).
function walkHarnessSources(dir = HARNESS_DIR) {
  const out = [];
  for (const ent of readdirSync(dir, { withFileTypes: true })) {
    if (ent.name === '__pycache__') continue;
    const abs = join(dir, ent.name);
    if (ent.isDirectory()) { out.push(...walkHarnessSources(abs)); continue; }
    if (!/\.(mjs|py|yml)$/.test(ent.name)) continue; // README.md etc. omitted
    out.push(relative(SOURCE_ROOT, abs));
  }
  return out;
}

// Is relPath covered by one of the copied GOVERNANCE_DIRS trees? (Future-proofs
// the guard if a harness asset ever moves under a copied .agent/ subtree.)
const underGovernanceDir = (relPath) =>
  GOVERNANCE_DIRS.some((d) => relPath === d || relPath.startsWith(d + sep));

test('COPIED_FILES has no drift: every harness source is copied or whitelisted', () => {
  const copied = new Set(COPIED_FILES);
  const whitelist = new Set(HARNESS_NOT_COPIED);
  const sources = walkHarnessSources();
  // Sanity: the walk actually found files (guards against a broken walker
  // vacuously passing) and the known drift fixture is now present in the manifest.
  assert.ok(sources.length > 10, `walk should find the harness sources; got ${sources.length}`);
  assert.ok(copied.has(join('harness', 'test_enforce.mjs')), 'the previously-dropped test_enforce.mjs must now be in COPIED_FILES');

  const missing = sources.filter(
    (rel) => !copied.has(rel) && !whitelist.has(rel) && !underGovernanceDir(rel),
  );
  assert.deepEqual(
    missing, [],
    `harness source(s) neither in COPIED_FILES nor whitelisted (drift — a scaffold ` +
    `would silently miss these):\n  ${missing.join('\n  ')}`,
  );

  // Honesty the other direction: the whitelist must name only REAL, present files
  // (a stale whitelist entry would hide a genuinely-missing copy behind a typo).
  for (const rel of whitelist) {
    assert.ok(existsSync(join(SOURCE_ROOT, rel)), `whitelisted ${rel} must exist (stale whitelist entry)`);
  }
});

test('forge-init exits non-zero on missing required args', () => {
  const noName = runInit([join(tmpdir(), 'forge-init-noname')]);
  assert.equal(noName.status, 2, 'missing --name must exit 2');
  assert.match(noName.stderr, /--name/);

  const noTarget = runInit(['--name', 'x']);
  assert.equal(noTarget.status, 2, 'missing <target-dir> must exit 2');
  assert.match(noTarget.stderr, /target-dir/);
});
