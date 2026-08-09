// Legacy-upgrade regression for Agent Engineering's shadow activation contract.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync,
} from 'node:fs';
import { dirname, join } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';

import { run, seedProjectInstances } from './forge-upgrade.mjs';

const SCAFFOLD_DIR = dirname(fileURLToPath(import.meta.url));
const SOURCE_ROOT = dirname(dirname(SCAFFOLD_DIR));
const INIT_PATH = join(SCAFFOLD_DIR, 'forge-init.mjs');
const LEGACY_ENGINEERING_FILES = [
  join('.agent', 'engineering'),
  ...['completion-evidence', 'backend-decision-package', 'frontend-design-package']
    .map((name) => join('.agent', 'eval', `${name}.schema.yml`)),
  ...[
    'backend-engineering', 'domain-modeling', 'data-modeling-transactions',
    'data-migration-lifecycle', 'api-contract-design', 'distributed-reliability-design',
    'performance-capacity', 'observability-engineering', 'architecture-tradeoff',
    'secure-coding', 'information-interaction-design', 'design-system-accessibility',
    'frontend-client-engineering',
    'frontend-code-architecture',
  ].map((name) => join('.agent', 'skills', `${name}.md`)),
  join('.arch', 'frontend-architecture.v1.json'),
  join('.arch', 'frontend-architecture-baseline.v1.json'),
  join('.arch', 'frontend-architecture-waivers.v1.json'),
  join('docs', 'design', 'ai-engineering-os'),
  join('docs', 'adr', '0042-frontend-design-decision-contract.md'),
  join('docs', 'adr', '0043-frontend-code-architecture-governance.md'),
  ...[
    'agent_engineering_check.py', 'backend_decision_contract.py',
    'backend_decision_check.py', 'backend_evidence_check.py', 'backend_package_check.py',
    'frontend_design_check.py', 'completion_evidence_check.py',
    'engineering_check_support.py', 'engineering_detector_check.py',
    'engineering_routing_check.py', 'test_agent_engineering_check.py',
    'test_backend_decision_check.py', 'test_frontend_design_adversarial.py',
    'test_frontend_design_check.py', 'test_legacy_ai_batch_contract.py',
  ].map((name) => join('harness', name)),
  ...['check.mjs', 'contract.mjs', 'graph.mjs', 'typescript-adapter.mjs', 'test_frontend-architecture.mjs']
    .map((name) => join('harness', 'frontend-architecture', name)),
  ...['__init__.py', 'contract.py', 'governance.py', 'evidence.py', 'model.py', 'package.py']
    .map((name) => join('harness', 'frontend_design', name)),
];

function scaffoldLegacyProject(t) {
  const root = mkdtempSync(join(tmpdir(), 'forge-upgrade-engineering-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const result = spawnSync(process.execPath, [INIT_PATH, target, '--name', 'legacy-project'], {
    encoding: 'utf8',
  });
  assert.equal(result.status, 0, result.stderr);
  const projectPath = join(target, '.agent', 'project.yml');
  const legacy = readFileSync(projectPath, 'utf8').replace(/\nengineering_spec:[\s\S]*$/, '\n');
  writeFileSync(projectPath, legacy);
  for (const relative of LEGACY_ENGINEERING_FILES) {
    rmSync(join(target, relative), { recursive: true, force: true });
  }
  return { target, projectPath, legacy };
}

test('legacy project upgrades to shadow contracts without rewriting project identity', (t) => {
  const { target, projectPath, legacy } = scaffoldLegacyProject(t);
  const result = run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false });
  assert.equal(result.applied, true);
  assert.equal(readFileSync(projectPath, 'utf8'), legacy, 'upgrade must preserve legacy project.yml');
  assert.equal(existsSync(join(target, '.agent', 'engineering', 'activation.yml')), true);
  assert.equal(existsSync(join(target, '.agent', 'engineering', 'backend-decision-gates.yml')), true);
  assert.equal(existsSync(join(target, '.agent', 'eval', 'backend-decision-package.schema.yml')), true);
  assert.equal(existsSync(join(target, '.agent', 'engineering', 'frontend-design-gates.yml')), true);
  assert.equal(existsSync(join(target, '.agent', 'engineering', 'frontend-profiles.yml')), true);
  assert.equal(existsSync(join(target, '.agent', 'eval', 'frontend-design-package.schema.yml')), true);
  assert.equal(existsSync(join(target, '.agent', 'skills', 'frontend-client-engineering.md')), true);
  assert.equal(existsSync(join(target, '.agent', 'skills', 'frontend-code-architecture.md')), true);
  assert.equal(existsSync(join(target, '.agent', 'skills', 'backend-engineering.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0042-frontend-design-decision-contract.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0043-frontend-code-architecture-governance.md')), true);
  assert.equal(existsSync(join(target, '.arch', 'frontend-architecture.v1.json')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend-architecture', 'check.mjs')), true);
  assert.equal(existsSync(join(target, 'harness', 'engineering_detector_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'backend_decision_contract.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'backend_decision_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'backend_evidence_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'backend_package_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design', '__init__.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design', 'contract.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design', 'governance.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design', 'evidence.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design', 'model.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design', 'package.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_frontend_design_adversarial.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_legacy_ai_batch_contract.py')), true);
  const check = spawnSync('python3', ['-B', 'harness/check.py', '.'], {
    cwd: target, encoding: 'utf8',
  });
  assert.equal(check.status, 0, `${check.stdout}\n${check.stderr}`);
  assert.match(check.stdout, /forge-check: PASS/);
});

test('upgrade never overwrites configured frontend architecture project instances', (t) => {
  const { target } = scaffoldLegacyProject(t);
  const configured = {
    contract: Buffer.from('{"project":"configured-targets"}\n'),
    baseline: Buffer.from('{"project":"owned-debt"}\n'),
    waivers: Buffer.from('{"project":"approved-exception"}\n'),
  };
  const paths = {
    contract: join(target, '.arch', 'frontend-architecture.v1.json'),
    baseline: join(target, '.arch', 'frontend-architecture-baseline.v1.json'),
    waivers: join(target, '.arch', 'frontend-architecture-waivers.v1.json'),
  };
  for (const [key, path] of Object.entries(paths)) writeFileSync(path, configured[key]);
  writeFileSync(join(target, 'harness', 'gate.mjs'), '// force a governed upgrade write\n');

  const result = run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false });
  for (const [key, path] of Object.entries(paths)) {
    assert.ok(readFileSync(path).equals(configured[key]), `${key} project instance must be preserved`);
  }
  assert.equal(result.projectInitialized, 0);
  assert.equal(result.drift.changed.some((rel) => rel.startsWith('.arch/frontend-architecture')), false);
});

test('stale missing-instance plan preserves a file created concurrently before seed', (t) => {
  const { target } = scaffoldLegacyProject(t);
  const relative = join('.arch', 'frontend-architecture.v1.json');
  const destination = join(target, relative);
  const concurrentBytes = Buffer.from('{"project":"created-by-another-agent"}\n');

  // The path was absent when the upgrade planned it. Another process wins the
  // create race before the planned seed executes.
  assert.equal(existsSync(destination), false);
  const stalePlan = [relative];
  writeFileSync(destination, concurrentBytes, { flag: 'wx' });

  const initialized = seedProjectInstances(stalePlan, SOURCE_ROOT, target);
  assert.equal(initialized, 0);
  assert.ok(readFileSync(destination).equals(concurrentBytes));
});
