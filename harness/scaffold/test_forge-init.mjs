// Tests for harness/scaffold/forge-init.mjs (node:test, zero external deps).
// Run: node --test harness/scaffold/test_forge-init.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  mkdtempSync,
  mkdirSync,
  rmSync,
  existsSync,
  readFileSync,
  readdirSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs';
import {
  dirname, join, relative, resolve, sep,
} from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';

import {
  COPIED_FILES,
  GOVERNANCE_DIRS,
  HARNESS_NOT_COPIED,
  PROJECT_INSTANCE_FILES,
  SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';

// This test lives in harness/scaffold/, so its own dir is the sub-package and the
// repo root is TWO levels up. HARNESS_DIR is the REAL harness/ (one level up) — the
// root the manifest-integrity guard walks (it must see EVERY harness source,
// including this sub-package).
const SCAFFOLD_DIR = dirname(fileURLToPath(import.meta.url));
const HARNESS_DIR = dirname(SCAFFOLD_DIR);
const SOURCE_ROOT = dirname(HARNESS_DIR);
const INIT_PATH = join(SCAFFOLD_DIR, 'forge-init.mjs');

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
  join('harness', 'agent_engineering_check.py'),
  join('harness', 'backend_decision_contract.py'),
  join('harness', 'backend_decision_check.py'),
  join('harness', 'backend_evidence_check.py'),
  join('harness', 'backend_package_check.py'),
  join('harness', 'frontend_design', '__init__.py'),
  join('harness', 'frontend_design', 'contract.py'),
  join('harness', 'frontend_design', 'composition.py'),
  join('harness', 'frontend_design', 'composition_support.py'),
  join('harness', 'frontend_design', 'geometry.py'),
  join('harness', 'frontend_design', 'governance.py'),
  join('harness', 'frontend_design_check.py'),
  join('harness', 'frontend_design', 'evidence.py'),
  join('harness', 'frontend_design', 'model.py'),
  join('harness', 'frontend_design', 'package.py'),
  join('harness', 'frontend_design_test_support.py'),
  join('harness', 'completion_evidence_check.py'),
  join('harness', 'engineering_detector_check.py'),
  join('harness', 'engineering_check_support.py'),
  join('harness', 'engineering_routing_check.py'),
  join('harness', 'governance_engineering_check.py'),
  join('harness', 'release_boundary_check.py'),
  join('harness', 'workflow_control_check.py'),
  join('harness', 'acceptance.mjs'),
  // Common + Rust/Java project adapters, declarations, and self-tests are all
  // inherited verbatim so a fresh project retains the same polyglot verdicts.
  join('harness', 'adapters.mjs'),
  join('harness', 'adapters', 'detection.mjs'),
  join('harness', 'adapters', 'project.mjs'),
  join('harness', 'adapters', 'go.yml'),
  join('harness', 'adapters', 'java.yml'),
  join('harness', 'adapters', 'python.yml'),
  join('harness', 'adapters', 'rust.yml'),
  join('harness', 'adapters', 'typescript.yml'),
  join('harness', 'yaml2json.py'),
  join('harness', 'scorecard.mjs'),
  join('harness', 'scorecard-update.mjs'),
  join('harness', 'test_check.py'),
  join('harness', 'test_check_bounded_input.py'),
  join('harness', 'test_agent_engineering_check.py'),
  join('harness', 'test_backend_decision_check.py'),
  join('harness', 'test_frontend_design_adversarial.py'),
  join('harness', 'test_frontend_business_ui_composition_boundaries.py'),
  join('harness', 'test_frontend_business_ui_geometry.py'),
  join('harness', 'test_frontend_geometry_coordinate_contract.py'),
  join('harness', 'test_frontend_design_check.py'),
  join('harness', 'test_legacy_ai_batch_contract.py'),
  join('harness', 'test_release_boundary_check.py'),
  join('harness', 'test_workflow_control_check.py'),
  join('harness', 'test_yaml2json.py'),
  join('harness', 'test_acceptance.mjs'),
  join('harness', 'test_adapters.mjs'),
  join('harness', 'test_polyglot_adapters.mjs'),
  join('harness', 'test_gate.mjs'),
  join('harness', 'test_scorecard.mjs'),
  join('harness', 'test_scorecard-update.mjs'),
  join('harness', 'test_secret-scan.mjs'),
  join('harness', 'arch', 'test_arch-check.mjs'),
];
// (c) Representative copied governance ASSETS consumed by check.py/acceptance.mjs.
const COPIED_ASSETS = [
  join('docs', 'release', 'README.md'),
  join('docs', 'design', 'ai-engineering-os', 'capability-catalog.v1.yml'),
  join('docs', 'design', 'ai-engineering-os', 'capability-skill-map.v1.yml'),
  join('docs', 'design', 'ai-engineering-os', 'backend-decision-standard.md'),
  join('docs', 'design', 'ai-engineering-os', 'frontend-design-standard.md'),
  join('docs', 'design', 'ai-engineering-os', 'frontend-code-architecture-standard.md'),
  join('docs', 'design', 'ai-engineering-os', 'governance-contracts.md'),
  join('docs', 'adr', '0042-frontend-design-decision-contract.md'),
  join('docs', 'adr', '0043-frontend-code-architecture-governance.md'),
  join('docs', 'adr', '0044-business-ui-geometry-contract.md'),
  join('docs', 'adr', '0046-local-governance-record-journal.md'),
  join('docs', 'contracts', 'governance-record-journal-v1.schema.json'),
  join('.agent', 'agents', 'architect.md'),
  join('.agent', 'agents', 'release-engineer.md'),
  join('.agent', 'skills', 'clean-architecture.md'),
  join('.agent', 'skills', 'backend-engineering.md'),
  join('.agent', 'skills', 'information-interaction-design.md'),
  join('.agent', 'skills', 'design-system-accessibility.md'),
  join('.agent', 'skills', 'frontend-client-engineering.md'),
  join('.agent', 'skills', 'frontend-code-architecture.md'),
  join('.agent', 'skills', 'ui-geometry.md'),
  join('.agent', 'workflows', 'build.yml'),
  join('.agent', 'workflows', 'deploy.yml'),
  join('.agent', 'workflows', 'rollback.yml'),
  join('.agent', 'eval', 'acceptance.schema.yml'),
  join('.agent', 'eval', 'completion-evidence.schema.yml'),
  join('.agent', 'eval', 'backend-decision-package.schema.yml'),
  join('.agent', 'eval', 'frontend-design-package.schema.yml'),
  join('.agent', 'engineering', 'activation.yml'),
  join('.agent', 'engineering', 'backend-decision-gates.yml'),
  join('.agent', 'engineering', 'frontend-design-gates.yml'),
  join('.agent', 'engineering', 'frontend-code-architecture.yml'),
  join('.agent', 'engineering', 'frontend-profiles.yml'),
  join('.arch', 'frontend-architecture.v1.json'),
  join('.arch', 'frontend-architecture-baseline.v1.json'),
  join('.arch', 'frontend-architecture-waivers.v1.json'),
  join('.agent', 'engineering', 'detectors.yml'),
  join('.agent', 'engineering', 'rules.yml'),
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
  join('examples', 'starter', 'package.json'),
  join('examples', 'starter', 'src', 'greet.mjs'),
  join('examples', 'starter', 'test', 'greet.test.mjs'),
  'README.md',
  '.gitignore',
  SCAFFOLD_STATE_FILE,
];

// Extract inline and reference-style Markdown destinations. This deliberately
// checks file reachability only: fragment correctness belongs to a Markdown
// linter, while scaffold integrity must prove every copied local file target is
// present in the generated project. `url` is the documented placeholder used by
// the researcher role card for per-result citations, not a repository path.
function markdownDestinations(markdown) {
  const destinations = [];
  for (const match of markdown.matchAll(/!?\[[^\]\n]*\]\(([^)\n]+)\)/g)) {
    destinations.push(match[1]);
  }
  for (const match of markdown.matchAll(/^\s*\[[^\]\n]+\]:\s*(\S+)/gm)) {
    destinations.push(match[1]);
  }
  return destinations;
}

function localMarkdownTarget(rawDestination) {
  let destination = rawDestination.trim();
  if (destination.startsWith('<')) {
    const closing = destination.indexOf('>');
    if (closing === -1) return null;
    destination = destination.slice(1, closing);
  } else {
    destination = destination.split(/\s+(?=["'])/, 1)[0];
  }
  if (
    destination === ''
    || destination.toLowerCase() === 'url'
    || destination.startsWith('#')
    || destination.startsWith('/')
    || destination.startsWith('//')
    || /^[A-Za-z][A-Za-z0-9+.-]*:/.test(destination)
  ) return null;
  const withoutFragment = destination.split('#', 1)[0].split('?', 1)[0];
  if (withoutFragment === '') return null;
  try {
    return decodeURIComponent(withoutFragment);
  } catch {
    return withoutFragment;
  }
}

function copiedMarkdownLinkIssues(target) {
  const state = JSON.parse(readFileSync(join(target, SCAFFOLD_STATE_FILE), 'utf8'));
  assert.ok(Array.isArray(state.copied), `${SCAFFOLD_STATE_FILE} must contain copied[]`);
  const targetRoot = resolve(target);
  const issues = [];
  for (const rel of state.copied.filter((path) => path.endsWith('.md'))) {
    const source = join(targetRoot, rel);
    const markdown = readFileSync(source, 'utf8');
    for (const rawDestination of markdownDestinations(markdown)) {
      const local = localMarkdownTarget(rawDestination);
      if (local === null) continue;
      const destination = resolve(dirname(source), local);
      const staysInsideTarget = destination === targetRoot
        || destination.startsWith(`${targetRoot}${sep}`);
      if (!staysInsideTarget || !existsSync(destination)) {
        issues.push(`${rel} -> ${rawDestination}`);
      }
    }
  }
  return issues;
}

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
  const scaffoldState = JSON.parse(readFileSync(join(target, SCAFFOLD_STATE_FILE), 'utf8'));
  for (const rel of PROJECT_INSTANCE_FILES) {
    assert.ok(existsSync(join(target, rel)), `missing project instance: ${rel}`);
    assert.equal(scaffoldState.copied.includes(rel), false, `${rel} must not enter the upgrade ledger`);
  }

  // (3) the generated CI gate runs `forge accept` (acceptance.mjs).
  assert.match(
    readFileSync(join(target, '.github', 'workflows', 'forge.yml'), 'utf8'),
    /node harness\/acceptance\.mjs/,
  );
  assert.match(
    readFileSync(join(target, '.github', 'workflows', 'forge.yml'), 'utf8'),
    /node-version: '22'/,
    'generated CI must satisfy harness/package.json node >=22',
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

  // (4b) Copied Markdown links must resolve inside the scaffold.
  assert.deepEqual(
    copiedMarkdownLinkIssues(target),
    [],
    'copied Markdown contains dangling or escaping local links',
  );
  assert.doesNotMatch(
    readFileSync(join(target, '.agent', 'skills', 'evidence-claim-management.md'), 'utf8'),
    /docs\/adr\/0037-capability-centric-ai-engineering-operating-model\.md/,
  );
  const journalSkillPath = join(target, '.agent', 'skills', 'evidence-claim-management.md');
  const journalSkill = readFileSync(journalSkillPath, 'utf8');
  assert.match(journalSkill, /forge-runtime governance journal show/);
  assert.match(journalSkill, /not_executed/);
  assert.doesNotMatch(journalSkill, /\bforge governance journal\b/);
  const copiedRuntime = scaffoldState.copied.some((rel) => rel.startsWith('forge-runtime/'));
  assert.equal(copiedRuntime, false, 'scaffold must not install the Rust runtime');

  // (5) Run the full acceptance gate; only its external PyYAML prerequisite may skip.
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

test('forge-init preflights README/CI conflicts before writing anything', (t) => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-init-conflict-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const target = join(dir, 'proj');
  const ci = join(target, '.github', 'workflows', 'forge.yml');
  mkdirSync(dirname(ci), { recursive: true });
  writeFileSync(join(target, 'README.md'), '# user readme\n');
  writeFileSync(ci, 'name: user-ci\n');

  const res = runInit([target, '--name', 'must-not-clobber']);
  assert.notEqual(res.status, 0, 'conflicting target must fail without --force');
  assert.match(res.stderr, /scaffold conflict/);
  assert.match(res.stderr, /README\.md/);
  assert.equal(readFileSync(join(target, 'README.md'), 'utf8'), '# user readme\n');
  assert.equal(readFileSync(ci, 'utf8'), 'name: user-ci\n');
  assert.equal(existsSync(join(target, '.agent')), false, 'preflight must happen before first write');
  assert.equal(existsSync(join(target, 'harness')), false, 'no copied harness may leak on failure');
});

test('forge-init refuses symlinked destination ancestors even with --force', (t) => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-init-symlink-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const target = join(dir, 'proj');
  const outside = join(dir, 'outside');
  mkdirSync(target, { recursive: true });
  mkdirSync(outside, { recursive: true });
  symlinkSync(outside, join(target, '.github'));

  const res = runInit([target, '--name', 'unsafe-link', '--force']);
  assert.notEqual(res.status, 0);
  assert.match(res.stderr, /unsafe symlink path.*\.github/i);
  assert.equal(existsSync(join(outside, 'workflows')), false, 'init must not write through the link');
});

test('forge-init refuses a symlink target and a symlink destination leaf', (t) => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-init-target-link-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const outside = join(dir, 'outside');
  const target = join(dir, 'project-link');
  mkdirSync(outside);
  symlinkSync(outside, target);

  const linkedTarget = runInit([target, '--name', 'unsafe-target', '--force']);
  assert.notEqual(linkedTarget.status, 0);
  assert.match(linkedTarget.stderr, /unsafe symlink path.*target directory/i);
  assert.equal(readdirSync(outside).length, 0, 'a symlink target must remain untouched');

  rmSync(target);
  mkdirSync(target);
  const outsideReadme = join(outside, 'README.md');
  writeFileSync(outsideReadme, '# external\n');
  symlinkSync(outsideReadme, join(target, 'README.md'));
  const linkedLeaf = runInit([target, '--name', 'unsafe-leaf', '--force']);
  assert.notEqual(linkedLeaf.status, 0);
  assert.match(linkedLeaf.stderr, /unsafe symlink path.*README\.md/i);
  assert.equal(readFileSync(outsideReadme, 'utf8'), '# external\n');
});

test('forge-init refuses a symlinked parent above the target directory', (t) => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-init-parent-link-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const realParent = join(dir, 'real-parent');
  const linkedParent = join(dir, 'linked-parent');
  mkdirSync(realParent);
  symlinkSync(realParent, linkedParent);

  const res = runInit([join(linkedParent, 'project'), '--name', 'unsafe-parent', '--force']);
  assert.notEqual(res.status, 0);
  assert.match(res.stderr, /unsafe symlink path.*target directory/i);
  assert.equal(readdirSync(realParent).length, 0, 'init must not create through a linked parent');
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
