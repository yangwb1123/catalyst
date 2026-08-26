import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  chmodSync, copyFileSync, existsSync, linkSync, lstatSync, mkdirSync,
  mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

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
  assertAuthenticatedADRLifecycleAuthorityEvidenceProjection,
  assertAuthenticatedADRLifecycleAuthorityEvidenceScaffold,
  assertNoAuthenticatedADRLifecycleAuthorityEvidenceInstall,
  AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_FORBIDDEN_INSTALLS,
} from './authenticated-adr-lifecycle-authority-evidence-upgrade-verification.mjs';
import {
  assertAuthenticatedADRLifecycleScaffold,
} from './authenticated-adr-lifecycle-upgrade-verification.mjs';
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
const MUTATION_TARGET = join('harness', 'governance_engineering',
  'authenticated_adr_lifecycle_authority_evidence.py');
const V34_POLICY_SHA256 =
  'c9cf65ac22da1172d2d397c2530ce1e2b5b353e6b5dca00fa80f7d2fa30b4e48';
const V35_SURFACE_MARKERS = [
  'authenticated_adr_lifecycle_authority_evidence',
  'authenticated_adr_lifecycle_v1_go_authority',
  'authenticatedadrlifecycleauthority', 'authenticatedadrlifecyclecontract',
  'ADR-0084', 'ADR-0085', 'Registry v35', 'registry-v35', 'is_v35',
];
const V35_OWNER_FILES = [
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
  'harness/governance_engineering/authenticated_adr_lifecycle_candidate.py',
  'harness/governance_engineering/registry_contract.py',
  'harness/governance_engineering/test_adr_governance_portable.py',
  'harness/governance_engineering/test_authenticated_adr_approval_candidate.py',
  'harness/governance_engineering/test_authenticated_adr_lifecycle_candidate.py',
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
  ...V35_OWNER_FILES, ...REGISTRY_V36_OWNER_FILES,
  ...CONCURRENT_CURRENT_FILES, ...REGISTRY_V37_OWNER_FILES,
  ...REGISTRY_V39_SHARED_OWNER_FILES,
])];
const UPGRADE_ADDED_FILES = new Set([
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

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function hasV35Surface(text) {
  return V35_SURFACE_MARKERS.some((marker) => text.includes(marker))
    || /(?:version[^\n]{0,40}\b35\b|\b35\b[^\n]{0,40}version)/i.test(text);
}

function discoveredV35OwnerFiles(root) {
  const exact4 = new Set(
    AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES);
  return copiedProjection(SOURCE_ROOT).filter((relative) => (
    !exact4.has(relative)
    && existsSync(join(root, relative))
    && hasV35Surface(readFileSync(join(root, relative), 'utf8'))
  ));
}

function projectPolicyV34(text) {
  let projected = text.replace(
    '_with_catalyst_repository_only_authenticated_adr_lifecycle_v1_go_authority_evidence_and_source_only_governance_distribution',
    '',
  ).replace('version: 35\n', 'version: 34\n');
  projected = projected.replace(
    /\nauthenticated_adr_lifecycle_v1_go_authority_evidence:\n[\s\S]*?\narchitecture_decision_record_v2:/,
    '\narchitecture_decision_record_v2:',
  );
  projected = projected.split('\n').filter((line) => ![
    '  authenticated_adr_lifecycle_v1_go_authority_decision:',
    '  authenticated_adr_lifecycle_v1_go_authority_governance_decision:',
    '  authenticated_adr_lifecycle_v1_go_authority_decision_sha256:',
    '  authenticated_adr_lifecycle_v1_go_authority_manifest_sha256:',
    '  - ADR-0084 is only Proposed Catalyst-repository lifecycle authority evidence ',
  ].some((prefix) => line.startsWith(prefix))).join('\n');
  projected = projected.replace(
    /  authenticated_adr_lifecycle_v1_go_contract:\n[\s\S]*?  evidence_claim_portable_skill:/,
    '  evidence_claim_portable_skill:',
  );
  return projected;
}

function projectRegistryOwnerV34(text) {
  return text.replace(
    '5728a84f7668a7d13b089ea0869c29058aa66235b56aa235ae4c3c0621b03796',
    V34_POLICY_SHA256,
  ).replace(
    '        "structural_candidate_source_distribution_only_with_catalyst_repository_"\n'
      + '        "only_authenticated_adr_lifecycle_v1_go_authority_evidence_and_source_"\n'
      + '        "only_governance_distribution"',
    '        "structural_candidate_source_distribution_only"',
  ).replace('    "version": 35,', '    "version": 34,')
    .replace('    "authenticated_adr_lifecycle_v1_go_authority_evidence",\n', '')
    .replace(
      '    "authenticated_adr_lifecycle_v1_go_authority_decision_sha256":\n'
        + '        "docs/adr/ADR-0084-authenticated-architecture-decision-lifecycle-"\n'
        + '        "authority-service-v1.md",\n',
      '',
    );
}

function projectV34(relative, text) {
  if (relative === POLICY) return projectPolicyV34(text);
  let projected = text.replaceAll('is_v35', 'is_v34')
    .replaceAll('Registry v35', 'Registry v34')
    .replaceAll('registry-v35', 'registry-v34')
    .replaceAll('["version"], 35', '["version"], 34')
    .replaceAll('policy["version"], 35', 'policy["version"], 34')
    .replaceAll('self.policy["version"], 35', 'self.policy["version"], 34')
    .replaceAll('data.get("version") != 35', 'data.get("version") != 34');
  if (relative === '.agent/engineering/activation.yml') {
    projected = projected.split('\n').filter((line) =>
      !line.includes('authenticated_adr_lifecycle_v1_go_authority_')).join('\n');
  } else if (relative === '.agent/engineering/disciplines.yml') {
    projected = projected.replace(
      ', docs/adr/ADR-0084-authenticated-architecture-decision-lifecycle-authority-service-v1.md, docs/adr/ADR-0085-authenticated-architecture-decision-lifecycle-authority-evidence-and-source-distribution.md',
      '',
    );
  } else if (relative === 'harness/agent_engineering/contract.py') {
    projected = projected.replace(
      /    "authenticated_adr_lifecycle_v1_go_authority_decision":[\s\S]*?    "governance_contract_skill":/,
      '    "governance_contract_skill":',
    );
  } else if (relative === 'harness/governance_engineering/registry_contract.py') {
    projected = projectRegistryOwnerV34(projected);
  } else if (relative === 'harness/governance_engineering_check.py') {
    projected = projected.split('\n').filter((line) =>
      !line.includes('authenticated_adr_lifecycle_authority_evidence')).join('\n');
  } else if (relative === '.agent/AGENTS.md'
      || relative === '.agent/engineering/README.md') {
    projected = projected.split('\n').filter((line) =>
      !line.includes('Registry v35')
      && !line.includes('ADR-0085')
      && !line.includes('ADR-0084/0085')).join('\n');
  } else if (relative === 'docs/design/ai-engineering-os/governance-contracts.md') {
    projected = projected.replace(
      /\n### ADR-0085 Authenticated ADR Lifecycle Authority Evidence\n[\s\S]*$/,
      '\n',
    );
  }
  return projected;
}

function seedV34Projection(target) {
  const exact4 = AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES;
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  writeFileSync(statePath, renderScaffoldState(
    state.copied.filter((relative) => !exact4.includes(relative))));
  for (const relative of exact4) rmSync(join(target, relative), { force: true });
  for (const relative of V35_OWNER_FILES) {
    const path = join(target, relative);
    writeFileSync(path, projectV34(relative, readFileSync(path, 'utf8')));
  }
}

function assertV34Projection(target) {
  for (const relative of AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES) {
    assert.equal(existsSync(join(target, relative)), false,
      `v34 projection must not contain ${relative}`);
  }
  for (const relative of AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES) {
    assert.equal(existsSync(join(target, relative)), true,
      `v34 projection must retain lifecycle exact24 path ${relative}`);
  }
  const policy = readFileSync(join(target, POLICY));
  assert.equal(sha256(policy), V34_POLICY_SHA256,
    'v34-like policy must exactly reproduce the frozen Registry v34 bytes');
  assert.equal(hasV35Surface(policy.toString('utf8')), false);
  const residual = copiedProjection(SOURCE_ROOT).filter((relative) => {
    const path = join(target, relative);
    return existsSync(path) && hasV35Surface(readFileSync(path, 'utf8'));
  });
  assert.deepEqual(residual, [],
    'v34-like projection must contain no v35 authority block, ref, pin or wiring');
  const state = JSON.parse(readFileSync(
    join(target, SCAFFOLD_STATE_FILE), 'utf8'));
  for (const relative of AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES) {
    assert.equal(state.copied.includes(relative), true,
      `v34 ledger must retain exact24 ${relative}`);
  }
}

function restoreExact(target, relative) {
  const path = join(target, relative);
  rmSync(path, { force: true });
  copyFileSync(join(SOURCE_ROOT, relative), path);
  chmodSync(path, 0o644);
}

test('exact4 is closed, disjoint from exact24 and excludes every Go path', () => {
  const exact4 = AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES;
  assert.equal(exact4.length, 4);
  assert.equal(new Set(exact4).size, 4);
  const exact24 = new Set(AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES);
  assert.deepEqual(exact4.filter((relative) => exact24.has(relative)), []);
  assert.equal(exact4.some((relative) => relative.startsWith('forge-core/')), false);
});

test('authority files, symlinks, FIFOs, policy promotion and Go paths fail closed', (t) => {
  const target = mkdtempSync(join(tmpdir(), 'lifecycle-authority-negative-'));
  t.after(() => rmSync(target, { recursive: true, force: true }));
  const policy = join(target, POLICY);
  mkdirSync(dirname(policy), { recursive: true });
  const original = readFileSync(join(SOURCE_ROOT, POLICY));
  writeFileSync(policy, original);
  assert.doesNotThrow(
    () => assertNoAuthenticatedADRLifecycleAuthorityEvidenceInstall(target));
  for (const parts of AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_FORBIDDEN_INSTALLS) {
    const path = join(target, ...parts);
    mkdirSync(path, { recursive: true });
    assert.throws(
      () => assertNoAuthenticatedADRLifecycleAuthorityEvidenceInstall(target),
      /must not install/);
    rmSync(path, { recursive: true, force: true });
  }
  const forbidden = join(target, '.forge', 'lifecycle-authority');
  mkdirSync(dirname(forbidden), { recursive: true });
  for (const kind of ['file', 'symlink', 'fifo']) {
    if (kind === 'file') writeFileSync(forbidden, 'forbidden\n');
    else if (kind === 'symlink') symlinkSync('missing-authority', forbidden);
    else assert.equal(spawnSync('mkfifo', [forbidden]).status, 0);
    assert.throws(
      () => assertNoAuthenticatedADRLifecycleAuthorityEvidenceInstall(target),
      /must not install/);
    rmSync(forbidden);
  }
  const text = original.toString('utf8');
  for (const mutation of [
    text.replace('version: 39', 'version: 35'),
    text.replace('    copies_go_contract_or_authority: false',
      '    copies_go_contract_or_authority: true'),
    text.replace('    repository_source_mutation_or_accepted_document_generation: false',
      '    repository_source_mutation_or_accepted_document_generation: true'),
  ]) {
    assert.notEqual(mutation, text, 'policy mutation sentinel must exist');
    writeFileSync(policy, mutation);
    assert.throws(
      () => assertNoAuthenticatedADRLifecycleAuthorityEvidenceInstall(target));
  }
});

test('real v34-like projection has no v35 surface and upgrade restores exact4', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'lifecycle-authority-v34-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const initialized = initialize(target, 'lifecycle-authority-v34');
  assert.equal(initialized.status, 0, `${initialized.stdout}\n${initialized.stderr}`);
  seedRegistryV35Projection(target);
  assertRegistryV35Projection(target, SOURCE_ROOT);
  assert.deepEqual(discoveredV35OwnerFiles(target), V35_OWNER_FILES,
    'v35 shared owner inventory must remain explicit and closed');
  seedV34Projection(target);
  assertV34Projection(target);
  const restored = run({
    from: SOURCE_ROOT, target, apply: true, backup: true, prune: false,
  });
  assert.deepEqual([...restored.drift.added].sort(),
    [...AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES,
      ...LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES,
      ...KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES,
      ...KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
      ...DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES].sort());
  assert.deepEqual([...restored.drift.changed].sort(),
    [...UPGRADE_CHANGED_OWNER_FILES].sort());
  for (const relative of UPGRADE_CHANGED_OWNER_FILES) {
    assert.equal(restored.drift.changed.includes(relative), true,
      `v34 shared owner must migrate to v35: ${relative}`);
    assert.deepEqual(readFileSync(join(target, relative)),
      readFileSync(join(SOURCE_ROOT, relative)),
      `v35 owner bytes drifted: ${relative}`);
    assert.equal(lstatSync(join(target, relative)).mode & 0o777,
      lstatSync(join(SOURCE_ROOT, relative)).mode & 0o777,
      `v35 owner mode drifted: ${relative}`);
  }
  assert.doesNotThrow(
    () => assertAuthenticatedADRLifecycleAuthorityEvidenceScaffold(target));
  assert.doesNotThrow(() => assertAuthenticatedADRLifecycleScaffold(target));
  assert.doesNotThrow(() => assertLegacyGovernanceReadImportProjection(target));
  assert.doesNotThrow(() => assertKernelOperationalReferenceProjection(target));
  const state = JSON.parse(readFileSync(
    join(target, SCAFFOLD_STATE_FILE), 'utf8'));
  for (const relative of [
    ...AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES,
    ...LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES,
    ...KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES,
    ...KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
  ]) {
    assert.equal(state.copied.includes(relative), true, `ledger missing ${relative}`);
    assert.deepEqual(readFileSync(join(target, relative)),
      readFileSync(join(SOURCE_ROOT, relative)), `legacy bytes drifted: ${relative}`);
    const info = lstatSync(join(target, relative));
    assert.equal(info.mode & 0o777, 0o644, `legacy mode drifted: ${relative}`);
    assert.equal(info.nlink, 1, `legacy link count drifted: ${relative}`);
  }
});

test('fresh exact4 source projection is pinned and mutation closed', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'lifecycle-authority-fresh-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const initialized = initialize(target, 'lifecycle-authority-fresh');
  assert.equal(initialized.status, 0, `${initialized.stdout}\n${initialized.stderr}`);
  assert.doesNotThrow(
    () => assertAuthenticatedADRLifecycleAuthorityEvidenceScaffold(target));
  const path = join(target, MUTATION_TARGET);
  const original = readFileSync(path);
  writeFileSync(path, Buffer.concat([original, Buffer.from('\n# mutation\n')]));
  assert.throws(
    () => assertAuthenticatedADRLifecycleAuthorityEvidenceProjection(target),
    /byte-identical to source/);
  restoreExact(target, MUTATION_TARGET);
  chmodSync(path, 0o777);
  assert.throws(
    () => assertAuthenticatedADRLifecycleAuthorityEvidenceProjection(target),
    /mode 0644/);
  chmodSync(path, 0o644);
  const alias = join(target, 'authority-evidence-alias.py');
  copyFileSync(path, alias);
  rmSync(path);
  linkSync(alias, path);
  assert.throws(
    () => assertAuthenticatedADRLifecycleAuthorityEvidenceProjection(target),
    /hardlink/);
  rmSync(path);
  rmSync(alias);
  symlinkSync(join(SOURCE_ROOT, MUTATION_TARGET), path);
  assert.throws(
    () => assertAuthenticatedADRLifecycleAuthorityEvidenceProjection(target),
    /symlink/);
  restoreExact(target, MUTATION_TARGET);
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  state.copied = state.copied.filter((relative) => relative !== MUTATION_TARGET);
  writeFileSync(statePath, renderScaffoldState(state.copied));
  assert.throws(
    () => assertAuthenticatedADRLifecycleAuthorityEvidenceProjection(target),
    /ledger entry missing/);
});
