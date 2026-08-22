// Test-only inverse projection of current Registry v39 through frozen v38/v37
// into frozen Registry v36. It proves the inverse chain owns every shared byte.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import {
  existsSync, lstatSync, readFileSync, rmSync, rmdirSync, writeFileSync,
} from 'node:fs';
import { join } from 'node:path';

import {
  KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES,
} from './kernel-operational-reference-copy-fragment.mjs';
import {
  KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
} from './kernel-decision-reference-copy-fragment.mjs';
import {
  REGISTRY_V38_OWNER_FILES,
  seedRegistryV37Projection,
} from './kernel-decision-reference-v37-projection.mjs';
import {
  discoveredRegistryV39OwnerFiles, REGISTRY_V39_OWNER_FILES,
  REGISTRY_V39_PRIOR_OWNER_FILES,
} from './decision-capsule-structural-replay-v38-projection.mjs';
import {
  copiedProjection, renderScaffoldState, SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';

const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const PYTHON_PACKAGE = join('harness', 'kernel_operational_contract');
export const REGISTRY_V36_POLICY_SHA256 =
  'ed0427dae8e81e233cab69691e8e47cb8443544d34e09d959fff691f7a77f924';
const REGISTRY_V37_POLICY_SHA256 =
  '1d8e8a0f3fff06c64e613b0e4c69533e10119d27dcaa7096e0e6be2578d308d5';
const V37_DISCIPLINE_ASSETS = [
  'docs/contracts/kernel-operational-reference-core-v1.schema.json',
  'docs/contracts/fixtures/kernel-operational-reference-closure-v1.json',
  'harness/kernel_operational_contract_check.py',
  'docs/adr/ADR-0088-kernel-operational-reference-core-v1.md',
  'docs/adr/ADR-0089-kernel-operational-reference-governance-and-source-distribution.md',
];
const V37_MARKERS = [
  'kernel_operational_reference', 'kernel-operational-reference',
  'Kernel operational reference', 'Kernel Operational Reference',
  'ADR-0089', 'Registry v37', 'registry-v37', 'is_v37',
];

export const REGISTRY_V37_OWNER_FILES = [
  '.agent/AGENTS.md',
  '.agent/engineering/README.md',
  '.agent/engineering/activation.yml',
  '.agent/engineering/detectors.yml',
  '.agent/engineering/disciplines.yml',
  '.agent/engineering/governance-contracts.yml',
  '.arch/rules.yaml',
  'docs/design/ai-engineering-os/governance-contracts.md',
  'harness/agent_engineering/contract.py',
  'harness/evolve_locator_observation_producer/test_governance.py',
  'harness/governance_engineering/authenticated_adr_approval_candidate.py',
  'harness/governance_engineering/authenticated_adr_lifecycle_authority_evidence.py',
  'harness/governance_engineering/authenticated_adr_lifecycle_candidate.py',
  'harness/governance_engineering/legacy_governance_read_import_candidate.py',
  'harness/governance_engineering/registry_contract.py',
  'harness/governance_engineering/test_adr_governance_portable.py',
  'harness/governance_engineering/test_authenticated_adr_approval_candidate.py',
  'harness/governance_engineering/test_authenticated_adr_lifecycle_authority_evidence.py',
  'harness/governance_engineering/test_authenticated_adr_lifecycle_candidate.py',
  'harness/governance_engineering/test_change_impact_cost_risk_portable.py',
  'harness/governance_engineering/test_context_package.py',
  'harness/governance_engineering/test_evidence_claim_portable.py',
  'harness/governance_engineering/test_go_package_dependency_graph_observation_producer.py',
  'harness/governance_engineering/test_knowledge_graph_curation_portable.py',
  'harness/governance_engineering/test_knowledge_update_proposal.py',
  'harness/governance_engineering/test_legacy_governance_read_import_candidate.py',
  'harness/governance_engineering/test_policy_authority_portable.py',
  'harness/governance_engineering/test_work_intent_candidate.py',
  'harness/governance_engineering/work_intent_candidate.py',
  'harness/governance_engineering_check.py',
  'harness/test_governance_engineering_integration.py',
  'harness/test_governance_evolve_locator_integration.py',
  'harness/test_governance_local_command_observation_producer_integration.py',
  'harness/test_local_go_package_impact_prescan_registry.py',
];

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function hasV37Surface(text) {
  return V37_MARKERS.some((marker) => text.includes(marker))
    || /(?:version[^\n]{0,40}\b37\b|\b37\b[^\n]{0,40}version)/i.test(text);
}

export function discoveredRegistryV37OwnerFiles(sourceRoot) {
  const postV36 = new Set([
    ...KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES,
    ...KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
  ]);
  return copiedProjection(sourceRoot).filter((relative) => (
    !postV36.has(relative)
    && hasV37Surface(readFileSync(join(sourceRoot, relative), 'utf8'))
  ));
}

function projectPolicyV36(text) {
  let projected = text.replace(
    '_and_proposed_kernel_operational_reference_core_v1_structural_candidate_'
      + 'with_source_only_python_distribution_and_catalyst_repository_only_go_'
      + 'and_rust_parity',
    '',
  ).replace('version: 37\n', 'version: 36\n');
  projected = projected.replace(
    /\nkernel_operational_reference_core_v1_candidate_contract:\n[\s\S]*?(?=\ncanonical_refs:)/,
    '',
  ).replace(
      /\n  kernel_operational_reference_core_v1_python:\n[\s\S]*?(?=\n  evidence_claim_portable_skill:)/,
      '',
    ).replace(/^  kernel_operational_reference_core_v1_.*\n/gm, '')
    .replace(/^  - ADR-0088\/0089 .*\n/m, '')
    .replace(
      'ADR-0086/0087 only project caller-supplied legacy Memory and ADR bytes '
        + 'into an unverified read-only view; Registry v37',
      'ADR-0086/0087 only project caller-supplied legacy Memory and ADR bytes '
        + 'into an unverified read-only view; Registry v36',
    );
  return projected;
}

function replaceRegistryVersionV36(text) {
  return text.replaceAll('is_v37', 'is_v36')
    .replaceAll('Registry v37', 'Registry v36')
    .replaceAll('registry-v37', 'registry-v36')
    .replaceAll('["version"], 37', '["version"], 36')
    .replaceAll('37, self.policy["version"]', '36, self.policy["version"]')
    .replaceAll('data.get("version") != 37', 'data.get("version") != 36');
}

function removeParagraph(text, marker) {
  const lines = text.split('\n');
  const index = lines.findIndex((line) => line.includes(marker));
  if (index === -1) return text;
  if (index > 0 && lines[index - 1] === '') lines.splice(index - 1, 2);
  else lines.splice(index, 1);
  return lines.join('\n');
}

function projectRegistryContractV36(text) {
  return text.replace(REGISTRY_V37_POLICY_SHA256, REGISTRY_V36_POLICY_SHA256)
    .replace(
      '_and_proposed_"\n'
        + '        "kernel_operational_reference_core_v1_structural_candidate_with_source_"\n'
        + '        "only_python_distribution_and_catalyst_repository_only_go_and_rust_parity',
      '',
    ).replace('    "version": 37,', '    "version": 36,')
    .replace('    "kernel_operational_reference_core_v1_candidate_contract",\n', '')
    .replace(
      /    "kernel_operational_reference_core_v1_schema_sha256":\n[\s\S]*?        "docs\/adr\/ADR-0088-kernel-operational-reference-core-v1\.md",\n/,
      '',
    );
}

function projectArchitectureV36(text) {
  const v37Block =
    '# 2026-08-15: ADR-0089 adds one cohesive governance integration module while\n'
      + '#              distributing the already-counted ADR-0088 checker. The scanner\n'
      + '#              measures harness at 52 non-test files, so max_files 52 → 53\n'
      + '#              preserves exactly one file of headroom.\n';
  const adr0090Block =
    '# 2026-08-15: Proposed ADR-0090 adds one universal pure Kernel-decision\n'
      + '#              contract checker while its validators and graph logic remain in\n'
      + '#              a cohesive sub-package. The scanner\'s current exact closure is\n'
      + '#              52 non-test files, so retaining max_files 53 preserves one-file\n'
      + '#              headroom.\n';
  const v37 =
    '  max_files: 53       # actual 52 non-test harness files '
      + '(through Proposed ADR-0090) + headroom 1';
  const v36 =
    '  max_files: 52       # actual 51 non-test harness files '
      + '(through Proposed ADR-0088) + headroom 1';
  assert.equal(text.includes(v37Block), true, 'v37 architecture owner block drifted');
  assert.equal(text.includes(adr0090Block), true,
    'v37 ADR-0090 architecture context drifted');
  assert.equal(text.includes(v37), true, 'v37 architecture budget drifted');
  return text.replace(v37Block, '').replace(adr0090Block, '').replace(v37, v36);
}

function projectDocumentationV36(relative, text) {
  if (relative === '.agent/AGENTS.md') {
    return removeParagraph(text, 'ADR-0089 在 active Registry v36');
  }
  if (relative === '.agent/engineering/README.md') {
    return removeParagraph(text, 'ADR-0089 advances the active policy');
  }
  if (relative !== 'docs/design/ai-engineering-os/governance-contracts.md') return null;
  return text.replace(
    /\n### ADR-0089 Kernel Operational Reference Governance\n[\s\S]*$/,
    '\n',
  );
}

function projectV36(relative, text) {
  if (relative === POLICY) return projectPolicyV36(text);
  let projected = replaceRegistryVersionV36(text);
  if (relative === '.agent/engineering/activation.yml') {
    return projected.split('\n').filter((line) =>
      !line.includes('kernel_operational_reference_')).join('\n');
  }
  if (relative === '.agent/engineering/detectors.yml') {
    return projected.replace(
      /\n  - id: governance\.kernel_operational_reference_core_v1_candidate\n[\s\S]*$/,
      '\n',
    );
  }
  if (relative === '.agent/engineering/disciplines.yml') {
    for (const asset of V37_DISCIPLINE_ASSETS) {
      projected = projected.replace(`, ${asset}`, '');
    }
    return projected;
  }
  if (relative === 'harness/agent_engineering/contract.py') {
    return projected.replace(
      /    "kernel_operational_reference_core_v1_schema": \([\s\S]*?    "governance_contract_skill":/,
      '    "governance_contract_skill":',
    );
  }
  if (relative === 'harness/governance_engineering/registry_contract.py') {
    return projectRegistryContractV36(projected);
  }
  if (relative === 'harness/governance_engineering_check.py') {
    return projected.split('\n').filter((line) =>
      !line.includes('kernel_operational_reference')).join('\n');
  }
  if (relative === 'harness/test_governance_engineering_integration.py') {
    return projected.replace(
      /\n    def test_kernel_operational_reference_drift_reaches_main_governance_aggregator\(self\):\n[\s\S]*?(?=\n    def )/,
      '\n',
    );
  }
  const documentation = projectDocumentationV36(relative, projected);
  if (documentation !== null) return documentation;
  if (relative === '.arch/rules.yaml') return projectArchitectureV36(projected);
  return projected;
}

export function seedRegistryV36Projection(target) {
  const policyPath = join(target, POLICY);
  if (sha256(readFileSync(policyPath)) === REGISTRY_V36_POLICY_SHA256) return;
  seedRegistryV37Projection(target);
  const exact18 = KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES;
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  writeFileSync(statePath, renderScaffoldState(
    state.copied.filter((relative) => !exact18.includes(relative))));
  for (const relative of exact18) rmSync(join(target, relative), { force: true });
  rmdirSync(join(target, PYTHON_PACKAGE));
  for (const relative of REGISTRY_V37_OWNER_FILES) {
    const path = join(target, relative);
    const before = readFileSync(path, 'utf8');
    const after = projectV36(relative, before);
    assert.notEqual(after, before, `v37 owner did not project to v36: ${relative}`);
    writeFileSync(path, after);
  }
}

export function assertRegistryV36Projection(target, sourceRoot) {
  assert.deepEqual(discoveredRegistryV39OwnerFiles(sourceRoot),
    REGISTRY_V39_OWNER_FILES,
    'current Registry v39 owner inventory must remain exact39 and closed');
  assert.deepEqual(REGISTRY_V38_OWNER_FILES, REGISTRY_V39_PRIOR_OWNER_FILES,
    'frozen v38 exact36 owner inventory must remain the v39 prior closure');
  for (const relative of KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES) {
    assert.equal(existsSync(join(target, relative)), false,
      `v36 projection must not contain exact18 ${relative}`);
  }
  const policy = readFileSync(join(target, POLICY));
  assert.equal(sha256(policy), REGISTRY_V36_POLICY_SHA256,
    'v36-like policy must exactly reproduce frozen Registry v36 bytes');
  assert.equal(hasV37Surface(policy.toString('utf8')), false);
  assert.equal(existsSync(join(target, PYTHON_PACKAGE)), false,
    'v36 projection must not retain the Python package directory');
  const residual = REGISTRY_V37_OWNER_FILES.filter((relative) =>
    relative !== '.arch/rules.yaml'
    && hasV37Surface(readFileSync(join(target, relative), 'utf8')));
  assert.deepEqual(residual, [], 'v36 projection must contain no v37 owner surface');
  const state = JSON.parse(readFileSync(join(target, SCAFFOLD_STATE_FILE), 'utf8'));
  assert.equal(state.copied.some((relative) =>
    KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES.includes(relative)), false,
  'v36 ledger must not claim any exact18 path');
  for (const relative of REGISTRY_V37_OWNER_FILES) {
    const info = lstatSync(join(target, relative));
    assert.equal(info.isFile(), true, `v36 owner must remain regular: ${relative}`);
    assert.equal(info.mode & 0o777,
      lstatSync(join(sourceRoot, relative)).mode & 0o777,
      `v36 owner mode drifted: ${relative}`);
    assert.equal(info.nlink, 1, `v36 owner link count drifted: ${relative}`);
  }
  const architecture = readFileSync(join(target, '.arch/rules.yaml'), 'utf8');
  assert.doesNotMatch(architecture, /ADR-0089/);
  assert.match(architecture, /^  max_files: 52\b/m);
}
