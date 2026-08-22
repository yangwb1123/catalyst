import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  chmodSync, copyFileSync, existsSync, linkSync, lstatSync, mkdirSync, mkdtempSync,
  readFileSync, rmSync, symlinkSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES,
} from './authenticated-adr-approval-copy-fragment.mjs';
import {
  AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES,
} from './authenticated-adr-lifecycle-copy-fragment.mjs';
import {
  AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES,
} from './authenticated-adr-lifecycle-authority-evidence-copy-fragment.mjs';
import {
  LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES,
} from './legacy-governance-read-import-copy-fragment.mjs';
import {
  KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
} from './kernel-decision-reference-copy-fragment.mjs';
import {
  KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES,
} from './kernel-operational-reference-copy-fragment.mjs';
import {
  assertAuthenticatedADRLifecycleProjection,
  assertAuthenticatedADRLifecycleScaffold,
  assertNoAuthenticatedADRLifecycleInstall,
  AUTHENTICATED_ADR_LIFECYCLE_CORE_TEST_ARGV,
  AUTHENTICATED_ADR_LIFECYCLE_FORBIDDEN_INSTALLS,
} from './authenticated-adr-lifecycle-upgrade-verification.mjs';
import {
  assertAuthenticatedADRLifecycleAuthorityEvidenceScaffold,
} from './authenticated-adr-lifecycle-authority-evidence-upgrade-verification.mjs';
import {
  assertLegacyGovernanceReadImportProjection,
} from './legacy-governance-read-import-upgrade-verification.mjs';
import {
  assertKernelOperationalReferenceProjection,
} from './kernel-operational-reference-upgrade-verification.mjs';
import {
  assertRegistryV35Projection,
  CONCURRENT_CURRENT_FILES,
  REGISTRY_V36_OWNER_FILES,
  seedRegistryV35Projection,
} from './legacy-governance-read-import-v35-projection.mjs';
import {
  REGISTRY_V37_OWNER_FILES,
} from './kernel-operational-reference-v36-projection.mjs';
import {
  REGISTRY_V39_SHARED_OWNER_FILES,
} from './decision-capsule-structural-replay-v38-projection.mjs';
import {
  DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES,
} from './decision-capsule-structural-replay-copy-fragment.mjs';
import {
  copiedProjection, renderScaffoldState, SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';
import { run } from './forge-upgrade.mjs';

const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const CORE = join('harness', 'authenticated_adr_lifecycle_contract', 'contract.py');
const PACKAGE = join('harness', 'authenticated_adr_lifecycle_contract');
const CURRENT_SURFACE_MARKERS = [
  'authenticated_adr_lifecycle', 'authenticated-adr-lifecycle',
  'authenticated architecture decision lifecycle', 'ADR-0081', 'ADR-0082',
  'ADR-0083', 'ADR-0084', 'ADR-0085', 'authenticatedadrlifecyclecontract',
  'architecture_decision_lifecycle',
];
const CURRENT_OWNER_FILES = [
  '.agent/AGENTS.md',
  '.agent/engineering/README.md',
  '.agent/engineering/activation.yml',
  '.agent/engineering/detectors.yml',
  '.agent/engineering/disciplines.yml',
  '.agent/engineering/governance-contracts.yml',
  'docs/design/ai-engineering-os/governance-contracts.md',
  'harness/agent_engineering/contract.py',
  'harness/evolve_locator_observation_producer/test_governance.py',
  'harness/governance_engineering/authenticated_adr_approval_candidate.py',
  'harness/governance_engineering/registry_contract.py',
  'harness/governance_engineering/test_adr_governance_portable.py',
  'harness/governance_engineering/test_authenticated_adr_approval_candidate.py',
  'harness/governance_engineering/test_change_impact_cost_risk_portable.py',
  'harness/governance_engineering/test_context_package.py',
  'harness/governance_engineering/test_evidence_claim_portable.py',
  'harness/governance_engineering/test_go_package_dependency_graph_observation_producer.py',
  'harness/governance_engineering/test_knowledge_graph_curation_portable.py',
  'harness/governance_engineering/test_knowledge_update_proposal.py',
  'harness/governance_engineering/test_policy_authority_portable.py',
  'harness/governance_engineering/test_work_intent_candidate.py',
  'harness/governance_engineering/work_intent_candidate.py',
  'harness/governance_engineering_check.py',
  'harness/test_governance_engineering_integration.py',
  'harness/test_governance_evolve_locator_integration.py',
  'harness/test_governance_local_command_observation_producer_integration.py',
  'harness/test_local_go_package_impact_prescan_registry.py',
];
const UPGRADE_OWNER_FILES = [...new Set([
  ...CURRENT_OWNER_FILES, ...REGISTRY_V36_OWNER_FILES,
  ...CONCURRENT_CURRENT_FILES, ...REGISTRY_V37_OWNER_FILES,
  ...REGISTRY_V39_SHARED_OWNER_FILES,
])];
const UPGRADE_ADDED_FILES = new Set([
  ...AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES,
  ...AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES,
  ...LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES,
  ...KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES,
  ...KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
  ...DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES,
]);
const UPGRADE_CHANGED_OWNER_FILES = UPGRADE_OWNER_FILES.filter(
  (relative) => !UPGRADE_ADDED_FILES.has(relative));

function initialize(target, name) {
  return spawnSync(process.execPath, [
    'harness/scaffold/forge-init.mjs', target, '--name', name,
  ], { cwd: SOURCE_ROOT, encoding: 'utf8' });
}

function withAmbientPythonPoison(root, action) {
  const poison = join(root, 'python-poison');
  mkdirSync(poison);
  writeFileSync(join(poison, 'jsonschema.py'),
    'raise RuntimeError("ambient jsonschema poison imported")\n');
  writeFileSync(join(poison, 'referencing.py'),
    'raise RuntimeError("ambient referencing poison imported")\n');
  const prior = {
    PYTHONPATH: process.env.PYTHONPATH,
    PYENV_VERSION: process.env.PYENV_VERSION,
  };
  process.env.PYTHONPATH = poison;
  process.env.PYENV_VERSION = 'authenticated-adr-lifecycle-invalid-python-version';
  try {
    return action();
  } finally {
    for (const [name, value] of Object.entries(prior)) {
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
    }
  }
}

function restoreCore(target) {
  const path = join(target, CORE);
  rmSync(path, { force: true });
  copyFileSync(join(SOURCE_ROOT, CORE), path);
  chmodSync(path, 0o644);
}

function hasCurrentSurface(text) {
  return CURRENT_SURFACE_MARKERS.some((marker) => text.includes(marker))
    || /(?:version[^\n]{0,40}\b35\b|\b35\b[^\n]{0,40}version)/i.test(text);
}

function discoveredCurrentOwnerFiles(root) {
  const delivered = new Set([
    ...AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES,
    ...AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES,
  ]);
  return copiedProjection(SOURCE_ROOT).filter((relative) => (
    !delivered.has(relative)
    && existsSync(join(root, relative))
    && hasCurrentSurface(readFileSync(join(root, relative), 'utf8'))
  ));
}

function seedV33Projection(target) {
  const delivered = [
    ...AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES,
    ...AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES,
  ];
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  const legacy = state.copied.filter(
    (relative) => !delivered.includes(relative));
  writeFileSync(statePath, renderScaffoldState(legacy));
  for (const relative of delivered) {
    rmSync(join(target, relative), { force: true });
  }
  for (const relative of CURRENT_OWNER_FILES) {
    const bytes = relative === POLICY
      ? 'api_version: forgeos.agent-engineering/v1\nkind: GovernanceContractRegistry\nversion: 33\n'
      : '# simulated registry-v33 owner without current lifecycle additions\n';
    writeFileSync(join(target, relative), bytes);
  }
}

function assertV33SurfaceAbsent(target) {
  const delivered = [
    ...AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES,
    ...AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES,
  ];
  for (const relative of delivered) {
    assert.equal(existsSync(join(target, relative)), false,
      `v33 projection must not contain ${relative}`);
  }
  const policy = readFileSync(join(target, POLICY), 'utf8');
  assert.match(policy, /^version: 33$/m);
  assert.equal(hasCurrentSurface(policy), false);
  const residual = copiedProjection(SOURCE_ROOT).filter((relative) => {
    const path = join(target, relative);
    return existsSync(path) && hasCurrentSurface(readFileSync(path, 'utf8'));
  });
  assert.deepEqual(residual, [],
    'v33 projection must have no lifecycle policy, activation, detector, pin or wiring residue');
  const routes = readFileSync(
    join(target, '.agent', 'engineering', 'context-routes.yml'), 'utf8');
  assert.equal(hasCurrentSurface(routes), false,
    'v33 context routes must contain no lifecycle route or identifier');
}

test('exact24 is disjoint from approval prerequisite ownership and excludes Go', () => {
  assert.equal(AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES.length, 24);
  assert.equal(new Set(AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES).size, 24);
  const approval = new Set(AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES);
  assert.deepEqual(
    AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES.filter((path) => approval.has(path)), []);
  assert.equal(AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES.some(
    (path) => path.startsWith('forge-core/') || path.includes('authenticatedadrlifecyclecontract')),
  false);
});

test('source-only boundary rejects authority, service, state, Skill and route nodes', (t) => {
  const target = mkdtempSync(join(tmpdir(), 'authenticated-adr-lifecycle-negative-'));
  t.after(() => rmSync(target, { recursive: true, force: true }));
  const policy = join(target, POLICY);
  mkdirSync(dirname(policy), { recursive: true });
  const original = readFileSync(join(SOURCE_ROOT, POLICY));
  writeFileSync(policy, original);
  assert.doesNotThrow(() => assertNoAuthenticatedADRLifecycleInstall(target));

  for (const parts of AUTHENTICATED_ADR_LIFECYCLE_FORBIDDEN_INSTALLS) {
    const forbidden = join(target, ...parts);
    mkdirSync(forbidden, { recursive: true });
    assert.throws(() => assertNoAuthenticatedADRLifecycleInstall(target),
      /must not install/);
    rmSync(forbidden, { recursive: true, force: true });
  }

  const authority = join(target, '.forge', 'authority');
  mkdirSync(dirname(authority), { recursive: true });
  writeFileSync(authority, 'forbidden\n');
  assert.throws(() => assertNoAuthenticatedADRLifecycleInstall(target),
    /must not install/);
  rmSync(authority);
  symlinkSync('missing-authority-target', authority);
  assert.throws(() => assertNoAuthenticatedADRLifecycleInstall(target),
    /must not install/);
  rmSync(authority);
  const fifo = spawnSync('mkfifo', [authority], { encoding: 'utf8' });
  assert.equal(fifo.status, 0, fifo.stderr);
  assert.throws(() => assertNoAuthenticatedADRLifecycleInstall(target),
    /must not install/);
  rmSync(authority);

  const text = original.toString('utf8');
  for (const mutation of [
    text.replace('version: 39', 'version: 35'),
    text.replace('    ed25519_or_signature_verification: false',
      '    ed25519_or_signature_verification: true'),
    text.replace('    copies_go_contract_or_authority: false',
      '    copies_go_contract_or_authority: true'),
    text.replace('    adds_command_socket_route_registry_scope_or_runtime_profile: false',
      '    adds_command_socket_route_registry_scope_or_runtime_profile: true'),
  ]) {
    assert.notEqual(mutation, text, 'policy mutation sentinel must be present');
    writeFileSync(policy, mutation);
    assert.throws(() => assertNoAuthenticatedADRLifecycleInstall(target));
  }
});

test('v33 projection upgrade restores exact28 and all current shared wiring', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'authenticated-adr-lifecycle-legacy-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const initialized = initialize(target, 'authenticated-adr-lifecycle-legacy');
  assert.equal(initialized.status, 0, `${initialized.stdout}\n${initialized.stderr}`);
  seedRegistryV35Projection(target);
  assertRegistryV35Projection(target, SOURCE_ROOT);
  assert.deepEqual(discoveredCurrentOwnerFiles(target), CURRENT_OWNER_FILES,
    'current shared owner inventory must remain explicit and closed');
  seedV33Projection(target);
  assertV33SurfaceAbsent(target);

  const restored = run({
    from: SOURCE_ROOT, target, apply: true, backup: true, prune: false,
  });
  assert.deepEqual([...restored.drift.added].sort(),
    [...AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES,
      ...AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES,
      ...LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES,
      ...KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES,
      ...KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
      ...DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES].sort());
  assert.deepEqual([...restored.drift.changed].sort(),
    [...UPGRADE_CHANGED_OWNER_FILES].sort());
  for (const relative of UPGRADE_CHANGED_OWNER_FILES) {
    assert.equal(restored.drift.changed.includes(relative), true,
      `v33 shared owner must migrate to current bytes: ${relative}`);
    assert.deepEqual(readFileSync(join(target, relative)),
      readFileSync(join(SOURCE_ROOT, relative)), `current owner bytes drifted: ${relative}`);
    assert.equal(lstatSync(join(target, relative)).mode & 0o777,
      lstatSync(join(SOURCE_ROOT, relative)).mode & 0o777,
      `current owner mode drifted: ${relative}`);
  }
  assert.doesNotThrow(() => assertAuthenticatedADRLifecycleScaffold(target));
  assert.doesNotThrow(
    () => assertAuthenticatedADRLifecycleAuthorityEvidenceScaffold(target));
  assert.doesNotThrow(() => assertLegacyGovernanceReadImportProjection(target));
  assert.doesNotThrow(() => assertKernelOperationalReferenceProjection(target));
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const recorded = JSON.parse(readFileSync(statePath, 'utf8')).copied;
  for (const relative of [
    ...AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES,
    ...AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES,
    ...LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES,
    ...KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES,
    ...KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
  ]) {
    assert.equal(recorded.includes(relative), true, `ledger missing ${relative}`);
    assert.deepEqual(readFileSync(join(target, relative)),
      readFileSync(join(SOURCE_ROOT, relative)), `legacy bytes drifted: ${relative}`);
    const stat = lstatSync(join(target, relative));
    assert.equal(stat.mode & 0o777, 0o644, `legacy mode drifted: ${relative}`);
    assert.equal(stat.nlink, 1, `legacy link count drifted: ${relative}`);
  }

  const core = join(target, CORE);
  chmodSync(core, 0o777);
  const unchanged = run({
    from: SOURCE_ROOT, target, apply: true, backup: true, prune: false,
  });
  assert.equal(unchanged.drift.unchanged.includes(CORE), true);
  assert.equal(lstatSync(core).mode & 0o777, 0o777,
    'generic byte upgrade must not be misrepresented as mode repair');
  assert.throws(() => assertAuthenticatedADRLifecycleProjection(target),
    /mode 0644; operator remediation required/);
  chmodSync(core, 0o644);
  assert.doesNotThrow(() => assertAuthenticatedADRLifecycleProjection(target));
});

test('fresh source-only scaffold is exact and mutation closed', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'authenticated-adr-lifecycle-fresh-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const result = initialize(target, 'authenticated-adr-lifecycle-focused');
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.deepEqual(AUTHENTICATED_ADR_LIFECYCLE_CORE_TEST_ARGV, [
    '-S', '-B', '-m', 'unittest', '-q',
    'harness.test_authenticated_adr_lifecycle_contract',
  ]);
  withAmbientPythonPoison(root,
    () => assert.doesNotThrow(() => assertAuthenticatedADRLifecycleScaffold(target)));

  const core = join(target, CORE);
  const coreBytes = readFileSync(core);
  writeFileSync(core, Buffer.concat([coreBytes, Buffer.from('\n# mutation\n')]));
  assert.throws(() => assertAuthenticatedADRLifecycleProjection(target),
    /byte-identical to source/);
  restoreCore(target);
  chmodSync(core, 0o777);
  assert.throws(() => assertAuthenticatedADRLifecycleProjection(target), /mode 0644/);
  chmodSync(core, 0o644);

  const alias = join(target, 'core-alias.py');
  copyFileSync(join(SOURCE_ROOT, CORE), alias);
  rmSync(core);
  linkSync(alias, core);
  assert.throws(() => assertAuthenticatedADRLifecycleProjection(target), /hardlink/);
  rmSync(core);
  rmSync(alias);
  symlinkSync(join(SOURCE_ROOT, CORE), core);
  assert.throws(() => assertAuthenticatedADRLifecycleProjection(target), /symlink/);
  restoreCore(target);
  copyFileSync(core, alias);
  linkSync(alias, join(target, PACKAGE, 'extra.py'));
  assert.throws(() => assertAuthenticatedADRLifecycleProjection(target), /hardlink/);
  rmSync(join(target, PACKAGE, 'extra.py'));
  rmSync(alias);
  for (const name of ['extra.py', 'extra.txt']) {
    const extra = join(target, PACKAGE, name);
    writeFileSync(extra, 'forbidden\n');
    assert.throws(() => assertAuthenticatedADRLifecycleProjection(target),
      /exact twelve-file physical closure/);
    rmSync(extra);
  }
  const extra = join(target, PACKAGE, 'extra-link');
  symlinkSync('missing', extra);
  assert.throws(() => assertAuthenticatedADRLifecycleProjection(target), /symlink/);
  rmSync(extra);

  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  state.copied = state.copied.filter((relative) => relative !== CORE);
  writeFileSync(statePath, renderScaffoldState(state.copied));
  assert.throws(() => assertAuthenticatedADRLifecycleProjection(target), /ledger entry missing/);
});
