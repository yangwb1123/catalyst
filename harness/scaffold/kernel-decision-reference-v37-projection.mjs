// Test-only inverse projection of current Registry v39 through frozen v38 into
// frozen Registry v37. It proves the historical inverse remains composable.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import {
  existsSync, lstatSync, readFileSync, rmSync, rmdirSync, writeFileSync,
} from 'node:fs';
import { join } from 'node:path';

import {
  KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
} from './kernel-decision-reference-copy-fragment.mjs';
import {
  renderScaffoldState, SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';
import {
  discoveredRegistryV39OwnerFiles, REGISTRY_V39_OWNER_FILES,
  REGISTRY_V39_PRIOR_OWNER_FILES, seedRegistryV38Projection,
} from './decision-capsule-structural-replay-v38-projection.mjs';

const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const PYTHON_PACKAGE = join('harness', 'kernel_decision_contract');
export const REGISTRY_V37_POLICY_SHA256 =
  'b9303b692950ebf47da9648c41950517403bd91a02b38c567c91581ad3556a70';
export const REGISTRY_V38_POLICY_SHA256 =
  '909fb4daa33f3854309b360f0f1ee4d0ba42d470d5edcbb54fff1be21f051752';
const V37_KERNEL_OPERATIONAL_GO_MANIFEST_SHA256 =
  'c9bfe4e6620b2e0e2e5db2da57e982a6d6b862881b2076f356f29bc02c379684';
const V38_KERNEL_OPERATIONAL_GO_MANIFEST_SHA256 =
  '009b38f62eb434d66e862761be15b55e8d49a5bdf71a2b685a1f3e9758986709';
const V37_KERNEL_OPERATIONAL_GO_CONTRACT_TEST_SHA256 =
  '011cb8083cbdd0debe8d06df7f22576834e505886284516fda64631781a22b09';
const V38_KERNEL_OPERATIONAL_GO_CONTRACT_TEST_SHA256 =
  'c0204815182f9b64d7422e1039ae498e7ff35b8cac8e7852c9f71546f5b475e6';
const V38_DISCIPLINE_ASSETS = [
  'docs/contracts/kernel-decision-reference-core-v1.schema.json',
  'docs/contracts/fixtures/kernel-decision-reference-closure-v1.json',
  'harness/kernel_decision_contract_check.py',
  'docs/adr/ADR-0090-kernel-decision-reference-core-v1.md',
  'docs/adr/ADR-0091-kernel-decision-reference-governance-and-source-distribution.md',
];
const V38_MARKERS = [
  'kernel_decision_reference', 'kernel-decision-reference',
  'Kernel decision reference', 'Kernel Decision Reference',
  'ADR-0091', 'Registry v38', 'registry-v38', 'is_v38',
];

export const REGISTRY_V38_OWNER_FILES = [
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
  'harness/governance_engineering/kernel_operational_reference_candidate.py',
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
  'harness/governance_engineering/test_kernel_operational_reference_candidate.py',
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

function hasV38Surface(text) {
  return V38_MARKERS.some((marker) => text.includes(marker))
    || /(?:version[^\n]{0,40}\b38\b|\b38\b[^\n]{0,40}version)/i.test(text);
}

function replaceRegistryVersionV37(text) {
  return text.replaceAll(REGISTRY_V38_POLICY_SHA256, REGISTRY_V37_POLICY_SHA256)
    .replaceAll('is_v38', 'is_v37')
    .replaceAll('Registry v38', 'Registry v37')
    .replaceAll('registry-v38', 'registry-v37')
    .replaceAll('version: 38\n', 'version: 37\n')
    .replaceAll('"version": 38', '"version": 37')
    .replaceAll('data.get("version") != 38', 'data.get("version") != 37')
    .replaceAll('data["version"], 38', 'data["version"], 37')
    .replaceAll('policy["version"], 38', 'policy["version"], 37')
    .replaceAll('self.policy["version"], 38', 'self.policy["version"], 37')
    .replaceAll('38, self.policy["version"]', '37, self.policy["version"]')
    .replaceAll('canonical v38 value', 'canonical v37 value');
}

function projectPolicyV37(text) {
  let projected = text.replace(
    '_and_proposed_kernel_decision_reference_core_v1_structural_candidate_'
      + 'with_source_only_python_distribution_and_catalyst_repository_only_go_'
      + 'and_rust_parity',
    '',
  );
  projected = projected.replace(
    /\nkernel_decision_reference_core_v1_candidate_contract:\n[\s\S]*?(?=\ncanonical_refs:)/,
    '',
  ).replace(
    /\n  kernel_decision_reference_core_v1_python:\n[\s\S]*?(?=\n  evidence_claim_portable_skill:)/,
    '',
  ).replace(/^  kernel_decision_reference_core_v1_.*\n/gm, '')
    .replace(/^  - ADR-0090\/0091 .*\n/m, '')
    .replace(V38_KERNEL_OPERATIONAL_GO_MANIFEST_SHA256,
      V37_KERNEL_OPERATIONAL_GO_MANIFEST_SHA256);
  return replaceRegistryVersionV37(projected);
}

function projectRegistryContractV37(text) {
  return replaceRegistryVersionV37(text).replace(
    '        "only_python_distribution_and_catalyst_repository_only_go_and_rust_parity_"\n'
      + '        "and_proposed_kernel_decision_reference_core_v1_structural_candidate_"\n'
      + '        "with_source_only_python_distribution_and_catalyst_repository_only_go_"\n'
      + '        "and_rust_parity"',
    '        "only_python_distribution_and_catalyst_repository_only_go_and_rust_parity"',
  ).replace(
    '    "kernel_decision_reference_core_v1_candidate_contract",\n',
    '',
  ).replace(
    /    "kernel_decision_reference_core_v1_schema_sha256":\n[\s\S]*?        "docs\/adr\/ADR-0090-kernel-decision-reference-core-v1\.md",\n/,
    '',
  );
}

function removeParagraph(text, marker) {
  const lines = text.split('\n');
  const index = lines.findIndex((line) => line.includes(marker));
  if (index === -1) return text;
  if (index > 0 && lines[index - 1] === '') lines.splice(index - 1, 2);
  else lines.splice(index, 1);
  return lines.join('\n');
}

function projectArchitectureV37(text) {
  const block =
    '# 2026-08-18: Proposed ADR-0091 adds one cohesive Kernel-decision governance\n'
      + '#              integration module while distributing the already-counted\n'
      + '#              ADR-0090 checker. The scanner measures harness at 53 non-test\n'
      + '#              files, so max_files 53 → 54 preserves one-file headroom.\n';
  const v38 =
    '  max_files: 54       # actual 53 non-test harness files '
      + '(through Proposed ADR-0091) + headroom 1';
  const v37 =
    '  max_files: 53       # actual 52 non-test harness files '
      + '(through Proposed ADR-0090) + headroom 1';
  assert.equal(text.includes(block), true, 'v38 architecture owner block drifted');
  assert.equal(text.includes(v38), true, 'v38 architecture budget drifted');
  return replaceRegistryVersionV37(text.replace(block, '').replace(v38, v37));
}

function projectDocumentationV37(relative, text) {
  if (relative === '.agent/AGENTS.md') {
    return removeParagraph(text, 'ADR-0090/0091 的 repository slice 已在 active Registry v38');
  }
  if (relative === '.agent/engineering/README.md') {
    return removeParagraph(text, "ADR-0090/0091's repository slice is delivered in Registry v38");
  }
  if (relative !== 'docs/design/ai-engineering-os/governance-contracts.md') return null;
  return text.replace(
    /\n### ADR-0091 Kernel Decision Reference Governance\n[\s\S]*$/,
    '',
  );
}

function projectOperationalCandidateV37(text) {
  const current =
    '        "- [x] 冻结 Kernel structural reference-family ABI（structural only）：扩展 "\n'
      + '        "CognitiveAtom source/type/authority/hardness，并定义 DecisionTransaction "\n'
      + '        "及其对 InteractionEvent、CapabilityInvocation、ArtifactReceipt、"\n'
      + '        "ExecutionReceipt 的单向引用闭包；"';
  const prior =
    '        "- [ ] 冻结完整 Kernel ABI：扩展 CognitiveAtom source/type/authority/hardness，"\n'
      + '        "并定义 DecisionTransaction、InteractionEvent、CapabilityInvocation、"\n'
      + '        "Artifact/ExecutionReceipt；"';
  return replaceRegistryVersionV37(text).replace(current, prior)
    .replace('Kernel structural roadmap unreadable', 'full Kernel ABI roadmap unreadable')
    .replace(
      'narrow structural reference-family ABI must remain one exact completed item',
      'full Kernel ABI parent must remain one exact open item',
    )
    .replace(
      'entries = [line for line in text.splitlines()\n'
        + '               if "Kernel structural reference-family ABI" in line]',
      'entries = [line for line in text.splitlines() if "冻结完整 Kernel ABI" in line]',
    );
}

function projectOperationalSourceV37(text) {
  return projectOperationalCandidateV37(text)
    .replace(V38_KERNEL_OPERATIONAL_GO_MANIFEST_SHA256,
      V37_KERNEL_OPERATIONAL_GO_MANIFEST_SHA256)
    .replace(V38_KERNEL_OPERATIONAL_GO_CONTRACT_TEST_SHA256,
      V37_KERNEL_OPERATIONAL_GO_CONTRACT_TEST_SHA256);
}

function projectOperationalTestV37(text) {
  return replaceRegistryVersionV37(text)
    .replace(
      '            "- [x] 冻结 Kernel structural reference-family ABI",\n'
        + '            "- [ ] 冻结 Kernel structural reference-family ABI"), encoding="utf-8")',
      '            "- [ ] 冻结完整 Kernel ABI", "- [x] 冻结完整 Kernel ABI"), encoding="utf-8")',
    )
    .replace('remain one exact completed item', 'remain one exact open item');
}

function projectV37(relative, text) {
  if (relative === POLICY) return projectPolicyV37(text);
  if (relative === '.agent/engineering/activation.yml') {
    return replaceRegistryVersionV37(text).split('\n').filter((line) =>
      !line.includes('kernel_decision_reference_')).join('\n');
  }
  if (relative === '.agent/engineering/detectors.yml') {
    return replaceRegistryVersionV37(text).replace(
      /\n  - id: governance\.kernel_decision_reference_core_v1_candidate\n[\s\S]*$/,
      '',
    );
  }
  if (relative === '.agent/engineering/disciplines.yml') {
    let projected = replaceRegistryVersionV37(text);
    for (const asset of V38_DISCIPLINE_ASSETS) projected = projected.replace(`, ${asset}`, '');
    return projected;
  }
  if (relative === 'harness/agent_engineering/contract.py') {
    return replaceRegistryVersionV37(text).replace(
      /    "kernel_decision_reference_core_v1_schema": \([\s\S]*?    "governance_contract_skill":/,
      '    "governance_contract_skill":',
    );
  }
  if (relative === 'harness/governance_engineering/registry_contract.py') {
    return projectRegistryContractV37(text);
  }
  if (relative === 'harness/governance_engineering/kernel_operational_reference_candidate.py') {
    return projectOperationalSourceV37(text);
  }
  if (relative === 'harness/governance_engineering/test_kernel_operational_reference_candidate.py') {
    return projectOperationalTestV37(text);
  }
  if (relative === 'harness/governance_engineering_check.py') {
    return replaceRegistryVersionV37(text).split('\n').filter((line) =>
      !line.includes('kernel_decision_reference')).join('\n');
  }
  if (relative === 'harness/test_governance_engineering_integration.py') {
    return replaceRegistryVersionV37(text).replace(
      /\n    def test_kernel_decision_reference_drift_reaches_main_governance_aggregator\(self\):\n[\s\S]*?(?=\n    def )/,
      '\n',
    );
  }
  const documentation = projectDocumentationV37(relative, text);
  if (documentation !== null) return documentation;
  if (relative === '.arch/rules.yaml') return projectArchitectureV37(text);
  return replaceRegistryVersionV37(text);
}

export function seedRegistryV37Projection(target) {
  const policyPath = join(target, POLICY);
  if (sha256(readFileSync(policyPath)) === REGISTRY_V37_POLICY_SHA256) return;
  seedRegistryV38Projection(target);
  const exact19 = KERNEL_DECISION_REFERENCE_EXPECTED_FILES;
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  writeFileSync(statePath, renderScaffoldState(
    state.copied.filter((relative) => !exact19.includes(relative))));
  for (const relative of exact19) rmSync(join(target, relative), { force: true });
  rmdirSync(join(target, PYTHON_PACKAGE));
  for (const relative of REGISTRY_V38_OWNER_FILES) {
    const path = join(target, relative);
    const before = readFileSync(path, 'utf8');
    const after = projectV37(relative, before);
    assert.notEqual(after, before, `v38 owner did not project to v37: ${relative}`);
    writeFileSync(path, after);
  }
}

export function assertRegistryV37Projection(target, sourceRoot) {
  assert.deepEqual(discoveredRegistryV39OwnerFiles(sourceRoot),
    REGISTRY_V39_OWNER_FILES,
    'current Registry v39 owner inventory must remain exact39 and closed');
  assert.deepEqual(REGISTRY_V38_OWNER_FILES, REGISTRY_V39_PRIOR_OWNER_FILES,
    'frozen v38 exact36 owner inventory must remain the v39 prior closure');
  for (const relative of KERNEL_DECISION_REFERENCE_EXPECTED_FILES) {
    assert.equal(existsSync(join(target, relative)), false,
      `v37 projection must not contain exact19 ${relative}`);
  }
  const policy = readFileSync(join(target, POLICY));
  assert.equal(sha256(policy), REGISTRY_V37_POLICY_SHA256,
    'v37-like policy must exactly reproduce frozen Registry v37 bytes');
  assert.equal(hasV38Surface(policy.toString('utf8')), false);
  assert.equal(existsSync(join(target, PYTHON_PACKAGE)), false,
    'v37 projection must not retain the Python package directory');
  const residual = REGISTRY_V38_OWNER_FILES.filter((relative) =>
    relative !== '.arch/rules.yaml'
    && hasV38Surface(readFileSync(join(target, relative), 'utf8')));
  assert.deepEqual(residual, [], 'v37 projection must contain no v38 owner surface');
  const state = JSON.parse(readFileSync(join(target, SCAFFOLD_STATE_FILE), 'utf8'));
  assert.equal(state.copied.some((relative) =>
    KERNEL_DECISION_REFERENCE_EXPECTED_FILES.includes(relative)), false,
  'v37 ledger must not claim any exact19 path');
  for (const relative of REGISTRY_V38_OWNER_FILES) {
    const info = lstatSync(join(target, relative));
    assert.equal(info.isFile(), true, `v37 owner must remain regular: ${relative}`);
    assert.equal(info.mode & 0o777,
      lstatSync(join(sourceRoot, relative)).mode & 0o777,
      `v37 owner mode drifted: ${relative}`);
    assert.equal(info.nlink, 1, `v37 owner link count drifted: ${relative}`);
  }
  const architecture = readFileSync(join(target, '.arch/rules.yaml'), 'utf8');
  assert.doesNotMatch(architecture, /ADR-0091/);
  assert.match(architecture, /^  max_files: 53\b/m);
}
