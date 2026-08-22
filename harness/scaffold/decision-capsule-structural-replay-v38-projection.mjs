// Test-only inverse projection of current Registry v39 into frozen Registry v38.
// It proves forge-upgrade owns every shared v39 byte, including .arch/rules.yaml.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import {
  existsSync, lstatSync, readFileSync, rmSync, rmdirSync, writeFileSync,
} from 'node:fs';
import { join } from 'node:path';

import {
  DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES,
} from './decision-capsule-structural-replay-copy-fragment.mjs';
import {
  copiedProjection, renderScaffoldState, SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';

const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const PYTHON_PACKAGE = join('harness', 'decision_capsule_contract');
const PRIOR_DISTRIBUTED_MODULE =
  'harness/governance_engineering/kernel_decision_reference_candidate.py';
const PRIOR_DISTRIBUTED_TEST =
  'harness/governance_engineering/test_kernel_decision_reference_candidate.py';
export const REGISTRY_V38_POLICY_SHA256 =
  '63b44231ae33a9788177db0d348b94d76ef368a8bcec2c9d67f4dabc7dace271';
export const REGISTRY_V39_POLICY_SHA256 =
  '7f72243aab82625e75f0b0da9823bbd76d083dc39365dad8795ed526b11d9a54';
const V39_DISCIPLINE_ASSETS = [
  'docs/contracts/decision-capsule-structural-replay-core-v1.schema.json',
  'docs/contracts/fixtures/decision-capsule-structural-replay-v1.json',
  'harness/decision_capsule_contract_check.py',
  'docs/adr/ADR-0092-decision-capsule-structural-replay-core-v1.md',
  'docs/adr/ADR-0093-decision-capsule-structural-replay-governance-and-source-distribution.md',
];
const V39_MARKERS = [
  'decision_capsule_structural_replay', 'decision-capsule-structural-replay',
  'Decision Capsule structural replay', 'Decision Capsule Structural Replay',
  'ADR-0093', 'Registry v39', 'registry-v39', 'is_v39',
];

// Registry-v38's shared inventory was exact37. Its source-distributed Kernel
// decision governance pair also survives v39 -> v38 and makes current ownership
// exact39 because all three files carry the current Registry-version surface.
export const REGISTRY_V39_PRIOR_OWNER_FILES = [
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
export const REGISTRY_V39_SHARED_OWNER_FILES = [
  ...REGISTRY_V39_PRIOR_OWNER_FILES,
  'harness/engineering_check_support.py',
  PRIOR_DISTRIBUTED_MODULE,
  PRIOR_DISTRIBUTED_TEST,
].sort();
export const REGISTRY_V39_OWNER_FILES = REGISTRY_V39_SHARED_OWNER_FILES;
export const REGISTRY_V38_OWNER_SHA256 = Object.freeze({
  '.agent/AGENTS.md': '12b3a4886ef30c70eb02a7df97c8f67bd4527bd5a2a05e6c550f117c34399c87',
  '.agent/engineering/README.md': 'ad127c07ed585f28c0cc5d767dd2cc506a091aea6666c3c8bb6ffb9e3d2addfb',
  '.agent/engineering/activation.yml': 'e2fe910060988c73af744beb57a37fb8f3ab1d8f018aa7593f52c8439c8c7556',
  '.agent/engineering/detectors.yml': '2c6a27c0e283cb4a11c96c5f5f82c6357d2d89bc9ee481623d5e73d4a3ad3bff',
  '.agent/engineering/disciplines.yml': '02069e36eb95466c1de87bdffb9c19122f05866421c5db8206297c05734e8924',
  '.agent/engineering/governance-contracts.yml': '63b44231ae33a9788177db0d348b94d76ef368a8bcec2c9d67f4dabc7dace271',
  '.arch/rules.yaml': 'b6f8b503254b199f7a2636ff2069857dc2555092ad3c32542d88a5f8452b38d3',
  'docs/design/ai-engineering-os/governance-contracts.md': '353bd372e8eb6a1142dd20be91ec912ef197a1ba50dc8ad57f32070c820eb977',
  'harness/agent_engineering/contract.py': '9872e852d37fe0d650ad2dc11558b31cd3417ae792ebd8bfa8a58ff7d18957dd',
  'harness/engineering_check_support.py': '0c4eec5cd5410c8d93091d9fd2bbfada4b9285521355502310bc69bc6a39af46',
  'harness/evolve_locator_observation_producer/test_governance.py': 'c2176a74ab6eb61023e7d82e91d182c5083315b9a92f2b9ee83b86d1dfeea7b3',
  'harness/governance_engineering/authenticated_adr_approval_candidate.py': 'c233f1bcdd6ab1e2bb7e0b6f76b2e4b6fb88c1e040957225e400b60e20f8fb3f',
  'harness/governance_engineering/authenticated_adr_lifecycle_authority_evidence.py': '9315725f5ea4490c32bfa2b2082c3eaa4557e1be75e69a9a20dca7af2a22ebd5',
  'harness/governance_engineering/authenticated_adr_lifecycle_candidate.py': '42c50dbdac8999bd90d78bd58a66225d5fe63cad65d103965078eac1162734fa',
  'harness/governance_engineering/kernel_decision_reference_candidate.py': 'a8d4ff8c2085b990bfb6c827968fc0402f5fde886f04611d3bac6aad0b07306b',
  'harness/governance_engineering/kernel_operational_reference_candidate.py': '11c5c0b294172315ee00776fa6ba8abd67fcdc54158a3ce4603b442b8404cb37',
  'harness/governance_engineering/legacy_governance_read_import_candidate.py': '5ae72ba7c119da30a7b946361581ccae1044b0c65d43f60bc99b178e3a12d2d8',
  'harness/governance_engineering/registry_contract.py': '0c6b5fb170d027cc1ab0d916eee42ede32dfe176101d170bb28a0410c3e6f14e',
  'harness/governance_engineering/test_adr_governance_portable.py': '7b59d730de8e5baf1f45cb10e772a55d7fb12ccce1bf450a610e3b5958cf3966',
  'harness/governance_engineering/test_authenticated_adr_approval_candidate.py': 'f919fd4633211aba41a34b5cbd7be1abc97f43e0784a461002ac153fbff4d478',
  'harness/governance_engineering/test_authenticated_adr_lifecycle_authority_evidence.py': '4abae645d3724b55db693452383e1d002de3b2bbd88e14c0d56cc7d47d0cda4c',
  'harness/governance_engineering/test_authenticated_adr_lifecycle_candidate.py': '30d59c9826bc5532194b7bb75ad54be83a9dec309549b6c9e7e0bb45916bdef4',
  'harness/governance_engineering/test_change_impact_cost_risk_portable.py': 'c47fcd062b11820466b8c067d512d9205e6ebeca808692dadeebee60f05244e9',
  'harness/governance_engineering/test_context_package.py': '09d846b269493aeb3b0837c3b679cc9b40768098b33d776d4d0a449eac9d8817',
  'harness/governance_engineering/test_evidence_claim_portable.py': '95a69e0f6329fb4d6a99d54ece90f756b1f9dcce108594bac4e2ddca3df098c2',
  'harness/governance_engineering/test_go_package_dependency_graph_observation_producer.py': '11d499141b3905e89c77679dc41c525ba66a850fa33ea9807c9ae2beb9e6b19b',
  'harness/governance_engineering/test_kernel_decision_reference_candidate.py': '9feb735c372b5c1c47fc606c37d1740e9007a80cb3f22fdf277ffd599519a4cc',
  'harness/governance_engineering/test_kernel_operational_reference_candidate.py': '0c5eec4fcb19475c5172b040b49accd3a9d6fcc8bf197d7dcbaffc18e89a392c',
  'harness/governance_engineering/test_knowledge_graph_curation_portable.py': '7aee573917ad92ce3d2892e139d9d435de173fd526f7c97eb0b6dede8b021cb2',
  'harness/governance_engineering/test_knowledge_update_proposal.py': '460708f939ee489efdd261be9e0cc755afc416b4635e29170b1c8d3738f95298',
  'harness/governance_engineering/test_legacy_governance_read_import_candidate.py': '008aaa9127c53314ab82047da8650df6ce52d1f794200f9ea8819f0af5375eb5',
  'harness/governance_engineering/test_policy_authority_portable.py': '0a9474716cc13339289a689020b49e8d457da37979f95cce3f67ee8820d167ed',
  'harness/governance_engineering/test_work_intent_candidate.py': '07ed8d695df619b027c6d63e8c59bb9b39b6bfcc6380bbf74df4836325b2ae9b',
  'harness/governance_engineering/work_intent_candidate.py': '252ddf954e7926f185548a5e39274740f013c7c30a257ea47f52d833b8ca51ca',
  'harness/governance_engineering_check.py': '7344def86f5ef00883e3f8fbf334148d0498be7ce199e5939ed91f940e24a31f',
  'harness/test_governance_engineering_integration.py': 'ea4fe8748f2b38f8ef90c3f9ca24754f1f8f9c8f51b3ba35e570511ff493b495',
  'harness/test_governance_evolve_locator_integration.py': 'e2c4f88688b5c08845f64f08313233f9ac40086a013d775a8fb0008b9e9327ac',
  'harness/test_governance_local_command_observation_producer_integration.py': '2a6a8c7b560f41d188692ed245cbc6a45f730296268ba41e60601650f67618ce',
  'harness/test_local_go_package_impact_prescan_registry.py': 'c3d80a78142da1619cdb3717b09b27026d20c7c1f02a7de90a28430e890190a3',
});

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function hasV39Surface(text) {
  return V39_MARKERS.some((marker) => text.includes(marker))
    || /(?:version[^\n]{0,40}\b39\b|\b39\b[^\n]{0,40}version)/i.test(text);
}

export function discoveredRegistryV39OwnerFiles(sourceRoot) {
  const exact19 = new Set(DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES);
  return copiedProjection(sourceRoot).filter((relative) => (
    !exact19.has(relative)
    && hasV39Surface(readFileSync(join(sourceRoot, relative), 'utf8'))
  ));
}

function replaceRegistryVersionV38(text) {
  return text.replaceAll(REGISTRY_V39_POLICY_SHA256, REGISTRY_V38_POLICY_SHA256)
    .replaceAll('is_v39', 'is_v38')
    .replaceAll('Registry v39', 'Registry v38')
    .replaceAll('registry-v39', 'registry-v38')
    .replaceAll('version: 39\n', 'version: 38\n')
    .replaceAll('"version": 39', '"version": 38')
    .replaceAll('data.get("version") != 39', 'data.get("version") != 38')
    .replaceAll('data["version"], 39', 'data["version"], 38')
    .replaceAll('policy["version"], 39', 'policy["version"], 38')
    .replaceAll('self.policy["version"], 39', 'self.policy["version"], 38')
    .replaceAll('39, self.policy["version"]', '38, self.policy["version"]')
    .replaceAll('canonical v39 value', 'canonical v38 value');
}

function projectPolicyV38(text) {
  let projected = text.replace(
    /_and_proposed_decision_capsule_structural_replay_core_v1_[a-z0-9_]+(?=\n)/,
    '',
  );
  projected = projected.replace(
    /\ndecision_capsule_structural_replay_core_v1_candidate_contract:\n[\s\S]*?(?=\ncanonical_refs:)/,
    '',
  ).replace(
    /\n  decision_capsule_structural_replay_core_v1_python:\n[\s\S]*?(?=\n  evidence_claim_portable_skill:)/,
    '',
  ).replace(/^  decision_capsule_structural_replay_core_v1_.*\n/gm, '')
    .replace(/^  - ADR-0092\/0093 .*\n/m, '');
  return replaceRegistryVersionV38(projected);
}

function projectRegistryContractV38(text) {
  const currentBinding =
    '        "and_rust_parity_and_proposed_decision_capsule_structural_replay_core_"\n'
      + '        "v1_structural_candidate_with_source_only_python_distribution_and_"\n'
      + '        "catalyst_repository_only_go_and_rust_parity"';
  const priorBinding = '        "and_rust_parity"';
  assert.equal(text.includes(currentBinding), true,
    'v39 Registry contract runtime binding drifted');
  let projected = replaceRegistryVersionV38(text).replace(
    currentBinding, priorBinding,
  ).replace(
    '    "decision_capsule_structural_replay_core_v1_candidate_contract",\n',
    '',
  ).replace(
    /    "decision_capsule_structural_replay_core_v1_schema_sha256":\n[\s\S]*?        "docs\/adr\/ADR-0092-decision-capsule-structural-replay-core-v1\.md",\n/,
    '',
  );
  // A formatter may keep short new pins on one line; remove any remaining exact
  // Decision Capsule pin entries while preserving every pre-v39 target.
  projected = projected.replace(
    /^    "decision_capsule_structural_replay_core_v1_[^"]+":.*\n/gm, '');
  return projected;
}

function removeParagraph(text, marker) {
  const lines = text.split('\n');
  const index = lines.findIndex((line) => line.includes(marker));
  if (index === -1) return text;
  if (index > 0 && lines[index - 1] === '') lines.splice(index - 1, 2);
  else lines.splice(index, 1);
  return lines.join('\n');
}

function projectArchitectureV38(text) {
  const decisionCapsuleBlock =
    '# 2026-08-20: Proposed ADR-0093 adds one cohesive Decision Capsule governance\n'
      + '#              integration module and three scaffold-owned source distribution\n'
      + '#              files. The scanner measures harness at 55 non-test files, so\n'
      + '#              max_files 54 → 56 preserves one-file headroom.\n';
  const hardeningBlock =
    '# 2026-08-21: independent review hardening separates descriptor-pinned parent\n'
      + '#              boundaries, added-file reservations, and scaffold-ledger\n'
      + '#              reservations into three focused transaction modules. The scanner\n'
      + '#              measures harness/scaffold at 58 non-test files, so max_files\n'
      + '#              56 → 59 preserves the established one-file headroom.\n';
  const authorityBlock =
    '# 2026-08-21: second-review hardening extracts durable transaction/stage authority\n'
      + '#              into one focused scaffold transaction module. The scanner measures\n'
      + '#              harness/scaffold at 59 non-test files, so max_files 59 → 60 keeps\n'
      + '#              the established one-file headroom.\n';
  const v39 =
    '  max_files: 60       # actual 59 non-test harness/scaffold files + headroom 1';
  const v38 =
    '  max_files: 54       # actual 53 non-test harness files '
      + '(through Proposed ADR-0091) + headroom 1';
  assert.equal(text.includes(decisionCapsuleBlock), true,
    'v39 architecture owner block drifted');
  assert.equal(text.includes(hardeningBlock), true,
    'v39 architecture hardening block drifted');
  assert.equal(text.includes(authorityBlock), true,
    'v39 architecture authority block drifted');
  assert.equal(text.includes(v39), true, 'v39 architecture budget drifted');
  return replaceRegistryVersionV38(text.replace(authorityBlock, '')
    .replace(hardeningBlock, '')
    .replace(decisionCapsuleBlock, '').replace(v39, v38));
}

function projectDocumentationV38(relative, text) {
  if (relative === '.agent/AGENTS.md') {
    return removeParagraph(text,
      'ADR-0092/0093 在 active Registry v39');
  }
  if (relative === '.agent/engineering/README.md') {
    return removeParagraph(text,
      'ADR-0092/0093 in Registry v39');
  }
  if (relative !== 'docs/design/ai-engineering-os/governance-contracts.md') {
    return null;
  }
  return text.replace(
    /\n### ADR-0093 Decision Capsule Structural Replay Governance\n[\s\S]*$/,
    '',
  );
}

function projectGovernanceCheckV38(text) {
  const currentImports =
    'from governance_engineering.kernel_operational_reference_candidate import '
      + 'integration_issues as _kernel_operational_reference_issues\n'
      + 'from governance_engineering.kernel_decision_reference_candidate import '
      + 'integration_issues as _kernel_decision_reference_issues\n'
      + 'from governance_engineering.decision_capsule_structural_replay_candidate '
      + 'import integration_issues as _decision_capsule_structural_replay_issues';
  const priorImports =
    'from governance_engineering.kernel_operational_reference_candidate import '
      + 'integration_issues as _kernel_operational_reference_issues\n'
      + 'from governance_engineering.kernel_decision_reference_candidate import '
      + 'integration_issues as _kernel_decision_reference_issues';
  const currentChecks =
    'CANDIDATE_INTEGRATION_CHECKS = (\n'
      + '    _work_intent_candidate_issues, _authenticated_adr_approval_candidate_issues, _authenticated_adr_lifecycle_candidate_issues, _authenticated_adr_lifecycle_authority_evidence_issues,\n'
      + '    _legacy_governance_read_import_issues, _kernel_operational_reference_issues, _kernel_decision_reference_issues, _decision_capsule_structural_replay_issues,\n'
      + ')\n';
  const currentDispatch =
    '    for candidate_check in CANDIDATE_INTEGRATION_CHECKS:\n'
      + '        issues.extend(candidate_check(data, path, repo_root, agent_root))';
  const priorDispatch = [
    '    issues.extend(_work_intent_candidate_issues(data, path, repo_root, agent_root))',
    '    issues.extend(_authenticated_adr_approval_candidate_issues(data, path, repo_root, agent_root))',
    '    issues.extend(_authenticated_adr_lifecycle_candidate_issues(data, path, repo_root, agent_root))',
    '    issues.extend(_authenticated_adr_lifecycle_authority_evidence_issues(data, path, repo_root, agent_root))',
    '    issues.extend(_legacy_governance_read_import_issues(data, path, repo_root, agent_root))',
    '    issues.extend(_kernel_operational_reference_issues(data, path, repo_root, agent_root))',
    '    issues.extend(_kernel_decision_reference_issues(data, path, repo_root, agent_root))',
  ].join('\n');
  assert.equal(text.includes(currentImports), true,
    'v39 governance aggregator imports drifted');
  assert.equal(text.includes(currentChecks), true,
    'v39 governance aggregator check inventory drifted');
  assert.equal(text.includes(currentDispatch), true,
    'v39 governance aggregator dispatch drifted');
  return replaceRegistryVersionV38(text)
    .replace(currentImports, priorImports)
    .replace(currentChecks, '')
    .replace(currentDispatch, priorDispatch);
}

const V39_BOUNDED_READER = `def read_bounded_spec(path):
    try:
        raw = read_regular_file(
            path, str(path), MAX_SPEC_BYTES,
            (0o600, 0o640, 0o644, 0o660, 0o664),
        )
    except MemoryError as error:
        raise ValueError("bounded spec read exhausted memory") from error
    except ValueError as error:
        if str(error).endswith(f" exceeds {MAX_SPEC_BYTES} bytes"):
            raise ValueError(f"file exceeds {MAX_SPEC_BYTES} bytes") from error
        raise
    if len(raw) > MAX_SPEC_BYTES:
        raise ValueError(f"file exceeds {MAX_SPEC_BYTES} bytes")
    return raw
`;
const V38_BOUNDED_READER = `def read_bounded_spec(path):
    try:
        with path.open("rb") as stream:
            if os.fstat(stream.fileno()).st_size > MAX_SPEC_BYTES:
                raise ValueError(f"file exceeds {MAX_SPEC_BYTES} bytes")
            raw = stream.read(MAX_SPEC_BYTES + 1)
    except MemoryError as error:
        raise ValueError("bounded spec read exhausted memory") from error
    if len(raw) > MAX_SPEC_BYTES:
        raise ValueError(f"file exceeds {MAX_SPEC_BYTES} bytes")
    return raw
`;

function projectEngineeringCheckSupportV38(text) {
  const imports =
    '# Registry v39 physical-source governance imports begin.\n'
      + 'import hashlib\n'
      + 'import stat\n'
      + '# Registry v39 physical-source governance imports end.\n';
  const helpers =
    /\n\n# Registry v39 physical-source governance helpers begin\.[\s\S]*# Registry v39 physical-source governance helpers end\.\n$/;
  assert.equal(text.includes(imports), true,
    'v39 shared physical-source imports drifted');
  assert.equal(text.includes(V39_BOUNDED_READER), true,
    'v39 bounded spec reader drifted');
  assert.match(text, helpers, 'v39 shared physical-source helpers drifted');
  return text.replace(imports, '').replace(V39_BOUNDED_READER, V38_BOUNDED_READER)
    .replace(helpers, '');
}

function projectV38(relative, text) {
  if (relative === POLICY) return projectPolicyV38(text);
  if (relative === '.agent/engineering/activation.yml') {
    return replaceRegistryVersionV38(text).split('\n').filter((line) =>
      !line.includes('decision_capsule_structural_replay_')).join('\n');
  }
  if (relative === '.agent/engineering/detectors.yml') {
    return replaceRegistryVersionV38(text).replace(
      /\n  - id: governance\.decision_capsule_structural_replay_core_v1_candidate\n[\s\S]*$/,
      '',
    );
  }
  if (relative === '.agent/engineering/disciplines.yml') {
    let projected = replaceRegistryVersionV38(text);
    for (const asset of V39_DISCIPLINE_ASSETS) {
      projected = projected.replace(`, ${asset}`, '');
    }
    return projected;
  }
  if (relative === 'harness/agent_engineering/contract.py') {
    return replaceRegistryVersionV38(text).replace(
      /    "decision_capsule_structural_replay_core_v1_schema": \([\s\S]*?    "governance_contract_skill":/,
      '    "governance_contract_skill":',
    );
  }
  if (relative === 'harness/governance_engineering/registry_contract.py') {
    return projectRegistryContractV38(text);
  }
  if (relative === 'harness/governance_engineering_check.py') {
    return projectGovernanceCheckV38(text);
  }
  if (relative === 'harness/engineering_check_support.py') {
    return projectEngineeringCheckSupportV38(text);
  }
  if (relative === 'harness/test_governance_engineering_integration.py') {
    return replaceRegistryVersionV38(text).replace(
      /\n    def test_decision_capsule_structural_replay_drift_reaches_main_governance_aggregator\(self\):\n[\s\S]*?(?=\n    def test_journal_schema_pin_is_enforced)/,
      '\n',
    );
  }
  const documentation = projectDocumentationV38(relative, text);
  if (documentation !== null) return documentation;
  if (relative === '.arch/rules.yaml') return projectArchitectureV38(text);
  return replaceRegistryVersionV38(text);
}

export function seedRegistryV38Projection(target) {
  const policyPath = join(target, POLICY);
  if (sha256(readFileSync(policyPath)) === REGISTRY_V38_POLICY_SHA256) return;
  const exact19 = DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES;
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  writeFileSync(statePath, renderScaffoldState(
    state.copied.filter((relative) => !exact19.includes(relative))));
  for (const relative of exact19) rmSync(join(target, relative), { force: true });
  rmdirSync(join(target, PYTHON_PACKAGE));
  for (const relative of REGISTRY_V39_SHARED_OWNER_FILES) {
    const path = join(target, relative);
    const before = readFileSync(path, 'utf8');
    const after = projectV38(relative, before);
    assert.notEqual(after, before, `v39 owner did not project to v38: ${relative}`);
    writeFileSync(path, after);
  }
}

export function assertRegistryV38Projection(target, sourceRoot) {
  assert.deepEqual(discoveredRegistryV39OwnerFiles(sourceRoot),
    REGISTRY_V39_OWNER_FILES,
    'Registry v39 owner inventory must remain explicit exact39 and closed');
  assert.deepEqual(Object.keys(REGISTRY_V38_OWNER_SHA256).sort(),
    REGISTRY_V39_SHARED_OWNER_FILES,
    'frozen Registry v38 owner manifest must cover exact39');
  for (const relative of DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES) {
    assert.equal(existsSync(join(target, relative)), false,
      `v38 projection must not contain exact19 ${relative}`);
  }
  const policy = readFileSync(join(target, POLICY));
  assert.equal(sha256(policy), REGISTRY_V38_POLICY_SHA256,
    'v38-like policy must exactly reproduce frozen Registry v38 bytes');
  assert.equal(hasV39Surface(policy.toString('utf8')), false);
  assert.equal(existsSync(join(target, PYTHON_PACKAGE)), false,
    'v38 projection must not retain the Decision Capsule Python package');
  const residual = REGISTRY_V39_SHARED_OWNER_FILES.filter((relative) =>
    relative !== '.arch/rules.yaml'
    && hasV39Surface(readFileSync(join(target, relative), 'utf8')));
  assert.deepEqual(residual, [], 'v38 projection must contain no v39 owner surface');
  const state = JSON.parse(readFileSync(join(target, SCAFFOLD_STATE_FILE), 'utf8'));
  assert.equal(state.copied.some((relative) =>
    DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES.includes(relative)), false,
  'v38 ledger must not claim any Decision Capsule exact19 path');
  for (const relative of REGISTRY_V39_SHARED_OWNER_FILES) {
    assert.equal(sha256(readFileSync(join(target, relative))),
      REGISTRY_V38_OWNER_SHA256[relative],
      `frozen Registry v38 owner bytes drifted: ${relative}`);
    const info = lstatSync(join(target, relative));
    assert.equal(info.isFile(), true, `v38 owner must remain regular: ${relative}`);
    assert.equal(info.mode & 0o777,
      lstatSync(join(sourceRoot, relative)).mode & 0o777,
      `v38 owner mode drifted: ${relative}`);
    assert.equal(info.nlink, 1, `v38 owner link count drifted: ${relative}`);
  }
  const architecture = readFileSync(join(target, '.arch/rules.yaml'), 'utf8');
  assert.doesNotMatch(architecture, /ADR-0093/);
  assert.match(architecture, /^  max_files: 54\b/m);
}
