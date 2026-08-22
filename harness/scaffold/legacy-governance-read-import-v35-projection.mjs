// Test-only inverse projection of current Registry v39 through v38/v37/v36
// into frozen Registry v35. It proves multi-generation upgrades stay deterministic.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import {
  existsSync, lstatSync, readFileSync, rmSync, rmdirSync, writeFileSync,
} from 'node:fs';
import { join } from 'node:path';

import {
  LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES,
} from './legacy-governance-read-import-copy-fragment.mjs';
import {
  copiedProjection, renderScaffoldState, SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';
import {
  seedRegistryV36Projection,
} from './kernel-operational-reference-v36-projection.mjs';
import {
  REGISTRY_V38_OWNER_FILES,
} from './kernel-decision-reference-v37-projection.mjs';
import {
  discoveredRegistryV39OwnerFiles, REGISTRY_V39_OWNER_FILES,
  REGISTRY_V39_PRIOR_OWNER_FILES,
} from './decision-capsule-structural-replay-v38-projection.mjs';
import {
  KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES,
} from './kernel-operational-reference-copy-fragment.mjs';

const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
export const REGISTRY_V35_POLICY_SHA256 =
  '6e768241b583ce96419974e7cf4051eb9196a1d7082f16b3a8fb12bcbf4762fb';
const REGISTRY_V36_POLICY_SHA256 =
  'ed0427dae8e81e233cab69691e8e47cb8443544d34e09d959fff691f7a77f924';
const PYTHON_PACKAGE = join('harness', 'legacy_governance_read_import_contract');
const OPERATIONAL_PACKAGE = join('harness', 'kernel_operational_contract');
export const CONCURRENT_CURRENT_FILES = ['.arch/rules.yaml'];
const ADR_GOVERNANCE =
  'harness/governance_engineering/adr_governance_portable.py';
const ADR_GOVERNANCE_TEST =
  'harness/governance_engineering/test_adr_governance_portable.py';
const LEGACY_ROADMAP_OPEN =
  '- [ ] 设计旧 memory/ADR 的只读导入';
const LEGACY_ROADMAP_CLOSED =
  '- [x] 设计旧 memory/ADR 的只读导入';
const V36_DISCIPLINE_ASSETS = [
  'docs/contracts/legacy-governance-read-import-v1.schema.json',
  'docs/contracts/fixtures/legacy-governance-read-import-memory-v1.jsonl',
  'docs/contracts/fixtures/legacy-governance-read-import-ADR-0001.md',
  'docs/contracts/fixtures/legacy-governance-read-import-ADR-0002.md',
  'docs/contracts/fixtures/legacy-governance-read-import-request-v1.json',
  'docs/contracts/fixtures/legacy-governance-read-import-view-v1.json',
  'harness/legacy_governance_read_import_contract_check.py',
  'docs/adr/ADR-0086-legacy-governance-read-only-import-v1.md',
  'docs/adr/ADR-0087-legacy-governance-read-import-governance-and-source-distribution.md',
];

const V36_SURFACE_MARKERS = [
  'legacy_governance_read_import', 'legacy-governance-read-import',
  'legacyGovernanceReadImport', 'ADR-0086', 'ADR-0087',
  'Registry v36', 'registry-v36', 'is_v36',
];

export const REGISTRY_V36_OWNER_FILES = [
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
  'harness/governance_engineering/authenticated_adr_lifecycle_authority_evidence.py',
  'harness/governance_engineering/authenticated_adr_lifecycle_candidate.py',
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
  'harness/governance_engineering/test_policy_authority_portable.py',
  'harness/governance_engineering/test_work_intent_candidate.py',
  'harness/governance_engineering/work_intent_candidate.py',
  'harness/governance_engineering_check.py',
  'harness/test_governance_engineering_integration.py',
  'harness/test_governance_evolve_locator_integration.py',
  'harness/test_governance_local_command_observation_producer_integration.py',
  'harness/test_local_go_package_impact_prescan_registry.py',
  ADR_GOVERNANCE,
  'harness/governance_engineering/architecture_decision_record_v2.py',
  'harness/governance_engineering/test_architecture_decision_record_v2.py',
];

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function hasV36Surface(text) {
  return V36_SURFACE_MARKERS.some((marker) => text.includes(marker))
    || /(?:version[^\n]{0,40}\b36\b|\b36\b[^\n]{0,40}version)/i.test(text);
}

export function discoveredRegistryV36OwnerFiles(sourceRoot) {
  const exact18 = new Set(LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES);
  const concurrent = new Set(CONCURRENT_CURRENT_FILES);
  const discovered = copiedProjection(sourceRoot).filter((relative) => (
    !exact18.has(relative)
    && !concurrent.has(relative)
    && hasV36Surface(readFileSync(join(sourceRoot, relative), 'utf8'))
  ));
  return [
    ...discovered,
    ADR_GOVERNANCE,
    'harness/governance_engineering/architecture_decision_record_v2.py',
    'harness/governance_engineering/test_architecture_decision_record_v2.py',
  ];
}

function projectPolicyV35(text) {
  let projected = text.replace(
    '_and_proposed_legacy_governance_read_import_v1_unverified_read_only_'
      + 'candidate_with_source_only_python_distribution_and_catalyst_'
      + 'repository_only_go_parity',
    '',
  ).replace('version: 36\n', 'version: 35\n')
    .replace(
      '  memory_import: explicit_supplied_bytes_unverified_read_only_projection_v1',
      '  memory_import: not_implemented',
    ).replace(
      '  adr_import: explicit_supplied_bytes_unverified_read_only_no_parse_projection_v1',
      '  adr_import: not_implemented',
    );
  projected = projected.replace(
    /\nlegacy_governance_read_import_v1_candidate_contract:\n[\s\S]*?(?=\ncanonical_refs:)/,
    '',
  ).replace(
    /\n  legacy_governance_read_import_v1_python:\n[\s\S]*?(?=\n  evidence_claim_portable_skill:)/,
    '',
  ).replace(/^  legacy_governance_read_import_v1_.*\n/gm, '')
    .replace(/^  - ADR-0086\/0087 .*\n/m, '');
  return projected;
}

function projectRegistryContractV35(text) {
  return text.replace(REGISTRY_V36_POLICY_SHA256, REGISTRY_V35_POLICY_SHA256)
    .replace(
      '        "only_governance_distribution_and_proposed_legacy_governance_read_"\n'
        + '        "import_v1_unverified_read_only_candidate_with_source_only_python_"\n'
        + '        "distribution_and_catalyst_repository_only_go_parity"',
      '        "only_governance_distribution"',
    ).replace('    "version": 36,', '    "version": 35,')
    .replace('    "legacy_governance_read_import_v1_candidate_contract",\n', '')
    .replace(
      /    "legacy_governance_read_import_v1_schema_sha256":\n[\s\S]*?        "docs\/adr\/ADR-0086-legacy-governance-read-only-import-v1\.md",\n/,
      '',
    );
}

function removeAddedParagraph(text, marker) {
  const lines = text.split('\n');
  const index = lines.findIndex((line) => line.includes(marker));
  if (index === -1) return text;
  if (index > 0 && lines[index - 1] === '') lines.splice(index - 1, 2);
  else lines.splice(index, 1);
  return lines.join('\n');
}

function projectDisciplineAssetsV35(text) {
  let projected = text;
  for (const relative of V36_DISCIPLINE_ASSETS) {
    projected = projected.replace(`, ${relative}`, '');
  }
  return projected;
}

function projectArchitectureV35(relative, text) {
  if (relative.endsWith('/architecture_decision_record_v2.py')) {
    return text.replace(
      '    if (_mapping(data.get("legacy")).get("adr_import") !=\n'
        + '            "explicit_supplied_bytes_unverified_read_only_no_parse_projection_v1"):\n'
        + '        issues.append(f"{path}: legacy ADR import must remain explicit and no-parse")',
      '    if _mapping(data.get("legacy")).get("adr_import") != "not_implemented":\n'
        + '        issues.append(f"{path}: legacy ADR import must remain not implemented")',
    );
  }
  return text.replace(
    '        mutated["legacy"]["adr_import"] = "not_implemented"\n', '',
  ).replace(
    '        self.assertTrue(any("legacy ADR import" in issue for issue in issues), issues)\n',
    '',
  );
}

function replaceRegistryVersionV35(text) {
  return text.replaceAll('is_v36', 'is_v35')
    .replaceAll('Registry v36', 'Registry v35')
    .replaceAll('registry-v36', 'registry-v35')
    .replaceAll('["version"], 36', '["version"], 35')
    .replaceAll('policy["version"], 36', 'policy["version"], 35')
    .replaceAll('self.policy["version"], 36', 'self.policy["version"], 35')
    .replaceAll('data.get("version") != 36', 'data.get("version") != 35');
}

function projectDocumentationV35(relative, text) {
  if (relative === '.agent/AGENTS.md') {
    return removeAddedParagraph(text, 'ADR-0087 unverified legacy read import');
  }
  if (relative === '.agent/engineering/README.md') {
    return removeAddedParagraph(text, 'ADR-0087 adds Registry v35 candidate');
  }
  if (relative === 'docs/design/ai-engineering-os/governance-contracts.md') {
    return text.replace(
      /\n### ADR-0087 Legacy Governance Read Import Governance\n[\s\S]*$/,
      '\n',
    );
  }
  return null;
}

function projectConcurrentArchitectureV35(text) {
  const currentBlock =
    '# 2026-08-14: ADR-0086 and Proposed ADR-0088 each add one universal pure\n'
    + '#              contract checker at the harness root; their validator, fixture,\n'
    + '#              and graph logic remain in cohesive sub-packages. The scanner\n'
    + '#              measures harness at 51 non-test files, so max_files 50 → 52\n'
    + '#              preserves exactly one file of headroom.\n';
  const currentBudget =
    '  max_files: 52       # actual 51 non-test harness files '
    + '(through Proposed ADR-0088) + headroom 1';
  const v35Budget =
    '  max_files: 50       # actual 49 non-test harness files '
    + '(pre-current baseline) + headroom 1';
  assert.equal(text.includes(currentBlock), true,
    'current concurrent architecture block must remain exact');
  assert.equal(text.includes(currentBudget), true,
    'current concurrent architecture budget must remain exact');
  return text.replace(currentBlock, '').replace(currentBudget, v35Budget);
}

function projectAdrGovernanceV35(relative, text) {
  if (relative === ADR_GOVERNANCE) {
    return text.replace(LEGACY_ROADMAP_CLOSED, LEGACY_ROADMAP_OPEN);
  }
  if (relative !== ADR_GOVERNANCE_TEST) return null;
  const projected = text.replaceAll(
    LEGACY_ROADMAP_CLOSED, LEGACY_ROADMAP_OPEN);
  return projected.replace(
    `self.assertNotIn("${LEGACY_ROADMAP_OPEN}", roadmap)`,
    `self.assertNotIn("${LEGACY_ROADMAP_CLOSED}", roadmap)`,
  );
}

function projectV35(relative, text) {
  if (relative === POLICY) return projectPolicyV35(text);
  const projected = replaceRegistryVersionV35(text);
  if (relative === '.agent/engineering/activation.yml') {
    return projected.split('\n').filter((line) =>
      !line.includes('legacy_governance_read_import_')).join('\n');
  }
  if (relative === '.agent/engineering/detectors.yml') {
    return projected.replace(
      /\n  - id: governance\.legacy_governance_read_import_v1_candidate\n[\s\S]*$/,
      '\n',
    );
  }
  if (relative === '.agent/engineering/disciplines.yml') {
    return projectDisciplineAssetsV35(projected);
  }
  if (relative === 'harness/agent_engineering/contract.py') {
    return projected.replace(
      /    "legacy_governance_read_import_v1_schema": \([\s\S]*?    "governance_contract_skill":/,
      '    "governance_contract_skill":',
    );
  }
  if (relative === 'harness/governance_engineering/registry_contract.py') {
    return projectRegistryContractV35(projected);
  }
  if (relative === 'harness/governance_engineering_check.py') {
    return projected.split('\n').filter((line) =>
      !line.includes('legacy_governance_read_import')).join('\n');
  }
  if (relative === 'harness/test_governance_engineering_integration.py') {
    return projected.replace(
      /\n    def test_legacy_import_core_drift_reaches_main_governance_aggregator\(self\):\n[\s\S]*?(?=\n    def )/,
      '\n',
    );
  }
  const documentation = projectDocumentationV35(relative, projected);
  if (documentation !== null) return documentation;
  const adrGovernance = projectAdrGovernanceV35(relative, projected);
  if (adrGovernance !== null) return adrGovernance;
  if (relative.includes('architecture_decision_record_v2.py')) {
    return projectArchitectureV35(relative, projected);
  }
  return projected;
}

export function seedRegistryV35Projection(target) {
  const policyPath = join(target, POLICY);
  if (sha256(readFileSync(policyPath)) === REGISTRY_V35_POLICY_SHA256) return;
  seedRegistryV36Projection(target);
  const exact18 = LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES;
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  writeFileSync(statePath, renderScaffoldState(
    state.copied.filter((relative) => !exact18.includes(relative))));
  for (const relative of exact18) rmSync(join(target, relative), { force: true });
  rmdirSync(join(target, PYTHON_PACKAGE));
  for (const relative of REGISTRY_V36_OWNER_FILES) {
    const path = join(target, relative);
    writeFileSync(path, projectV35(relative, readFileSync(path, 'utf8')));
  }
  const concurrent = join(target, CONCURRENT_CURRENT_FILES[0]);
  writeFileSync(concurrent,
    projectConcurrentArchitectureV35(readFileSync(concurrent, 'utf8')));
}

function assertV35OwnerFiles(target, sourceRoot) {
  const residual = REGISTRY_V36_OWNER_FILES.filter((relative) =>
    hasV36Surface(readFileSync(join(target, relative), 'utf8')));
  assert.deepEqual(residual, [],
    'v35-like projection must contain no Registry v36 owner surface');
  for (const relative of REGISTRY_V36_OWNER_FILES) {
    const info = lstatSync(join(target, relative));
    assert.equal(info.isFile(), true, `v35 owner must remain regular: ${relative}`);
    assert.equal(info.mode & 0o777,
      lstatSync(join(sourceRoot, relative)).mode & 0o777,
      `v35 owner mode drifted: ${relative}`);
    assert.equal(info.nlink, 1, `v35 owner link count drifted: ${relative}`);
  }
}

function assertV35Roadmap(target) {
  const adrGovernance = readFileSync(join(target, ADR_GOVERNANCE), 'utf8');
  const adrGovernanceTest = readFileSync(join(target, ADR_GOVERNANCE_TEST), 'utf8');
  assert.match(adrGovernance, /- \[ \] 设计旧 memory\/ADR 的只读导入/);
  assert.doesNotMatch(adrGovernance, /- \[x\] 设计旧 memory\/ADR 的只读导入/);
  assert.match(adrGovernanceTest,
    /self\.assertNotIn\("- \[x\] 设计旧 memory\/ADR 的只读导入", roadmap\)/);
  assert.match(adrGovernanceTest,
    /^            "- \[ \] 设计旧 memory\/ADR 的只读导入",$/m);
}

export function assertRegistryV35Projection(target, sourceRoot) {
  assert.deepEqual(CONCURRENT_CURRENT_FILES, ['.arch/rules.yaml'],
    'v35 inverse must isolate exactly one concurrent current file');
  assert.deepEqual(discoveredRegistryV39OwnerFiles(sourceRoot),
    REGISTRY_V39_OWNER_FILES,
    'current Registry v39 owner inventory must remain exact39 and closed');
  assert.deepEqual(REGISTRY_V38_OWNER_FILES, REGISTRY_V39_PRIOR_OWNER_FILES,
    'frozen v38 exact36 owner inventory must remain the v39 prior closure');
  for (const relative of [
    ...LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES,
    ...KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES,
  ]) {
    assert.equal(existsSync(join(target, relative)), false,
      `v35 projection must not contain exact18 ${relative}`);
  }
  const policy = readFileSync(join(target, POLICY));
  assert.equal(sha256(policy), REGISTRY_V35_POLICY_SHA256,
    'v35-like policy must exactly reproduce frozen Registry v35 bytes');
  assert.equal(hasV36Surface(policy.toString('utf8')), false);
  assert.match(policy.toString('utf8'), /^  missing_confidence_default: forbidden$/m);
  assert.match(policy.toString('utf8'), /^  legacy_status_is_authority: false$/m);
  assert.equal(existsSync(join(target, PYTHON_PACKAGE)), false,
    'v35 projection must not retain an empty legacy import package directory');
  assert.equal(existsSync(join(target, OPERATIONAL_PACKAGE)), false,
    'v35 projection must not retain an operational-reference package directory');
  const state = JSON.parse(readFileSync(join(target, SCAFFOLD_STATE_FILE), 'utf8'));
  assert.equal(state.copied.some((relative) =>
    LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES.includes(relative)
    || KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES.includes(relative)), false,
  'v35 ledger must not claim either post-v35 exact18 slice');
  assertV35OwnerFiles(target, sourceRoot);
  const concurrent = readFileSync(join(target, CONCURRENT_CURRENT_FILES[0]), 'utf8');
  assert.doesNotMatch(concurrent, /ADR-0086|ADR-0088/,
    'v35 concurrent architecture projection must predate both new checkers');
  assert.match(concurrent, /^  max_files: 50\b/m,
    'v35 concurrent architecture projection must restore max_files 50');
  assertV35Roadmap(target);
}
