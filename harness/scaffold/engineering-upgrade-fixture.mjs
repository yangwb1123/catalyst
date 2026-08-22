// Legacy Agent Engineering projection and per-slice upgrade assertions shared
// by the upgrade behavior test. This is scaffold-time test support only.
import { join } from 'node:path';

import { assertApprovalRecordUpgrade } from './approval-record-upgrade-verification.mjs';
import { assertTransitionReceiptUpgrade } from './transition-receipt-upgrade-verification.mjs';
import {
  assertKnowledgeUpdateProposalUpgrade,
  KNOWLEDGE_UPDATE_PROPOSAL_LEGACY_FILES,
} from './knowledge-update-proposal-upgrade-verification.mjs';
import {
  assertLocalGoPackageImpactPrescanUpgrade,
  LOCAL_GO_PACKAGE_IMPACT_PRESCAN_LEGACY_FILES,
} from './local-go-package-impact-prescan-upgrade-verification.mjs';
import {
  assertStrictReviewerScaffold,
  STRICT_REVIEWER_LEGACY_FILES,
} from './strict-reviewer-scaffold-verification.mjs';
import {
  assertGraphSnapshotScaffold,
  GRAPH_SNAPSHOT_LEGACY_FILES,
} from './graph-snapshot-upgrade-verification.mjs';
import { assertADRV2Scaffold, ADR_V2_LEGACY_FILES } from './adr-v2-upgrade-verification.mjs';
import {
  assertCapabilityRegistryScaffold,
  CAPABILITY_REGISTRY_LEGACY_FILES,
} from './capability-registry-upgrade-verification.mjs';
import {
  assertOwnershipProjectionScaffold,
  OWNERSHIP_PROJECTION_LEGACY_FILES,
} from './ownership-projection-upgrade-verification.mjs';
import {
  assertProjectSnapshotScaffold,
  PROJECT_SNAPSHOT_LEGACY_FILES,
} from './project-snapshot-upgrade-verification.mjs';
import {
  assertContextEngineeringScaffold,
  CONTEXT_ENGINEERING_LEGACY_FILES,
} from './context-engineering-upgrade-verification.mjs';
import {
  assertEvidenceClaimScaffold,
  EVIDENCE_CLAIM_LEGACY_FILES,
} from './evidence-claim-upgrade-verification.mjs';
import {
  assertPolicyAuthorityScaffold,
  POLICY_AUTHORITY_LEGACY_FILES,
} from './policy-authority-upgrade-verification.mjs';
import {
  assertADRGovernanceScaffold,
  ADR_GOVERNANCE_LEGACY_FILES,
} from './adr-governance-upgrade-verification.mjs';
import {
  assertKnowledgeGraphCurationScaffold,
  KNOWLEDGE_GRAPH_CURATION_LEGACY_FILES,
} from './knowledge-graph-curation-upgrade-verification.mjs';
import {
  assertChangeImpactCostRiskScaffold,
  CHANGE_IMPACT_COST_RISK_LEGACY_FILES,
} from './change-impact-cost-risk-upgrade-verification.mjs';
import {
  assertWorkIntentScaffold,
  WORK_INTENT_LEGACY_FILES,
} from './work-intent-upgrade-verification.mjs';
import {
  assertAuthenticatedADRApprovalScaffold,
  AUTHENTICATED_ADR_APPROVAL_LEGACY_FILES,
} from './authenticated-adr-approval-upgrade-verification.mjs';
import {
  assertAuthenticatedADRLifecycleScaffold,
  AUTHENTICATED_ADR_LIFECYCLE_LEGACY_FILES,
} from './authenticated-adr-lifecycle-upgrade-verification.mjs';
import {
  assertAuthenticatedADRLifecycleAuthorityEvidenceScaffold,
  AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_LEGACY_FILES,
} from './authenticated-adr-lifecycle-authority-evidence-upgrade-verification.mjs';
import {
  assertLegacyGovernanceReadImportScaffold,
  LEGACY_GOVERNANCE_READ_IMPORT_LEGACY_FILES,
} from './legacy-governance-read-import-upgrade-verification.mjs';
import {
  assertKernelOperationalReferenceScaffold,
  KERNEL_OPERATIONAL_REFERENCE_LEGACY_FILES,
} from './kernel-operational-reference-upgrade-verification.mjs';
import {
  assertKernelDecisionReferenceScaffold,
  KERNEL_DECISION_REFERENCE_LEGACY_FILES,
} from './kernel-decision-reference-upgrade-verification.mjs';

const SLICE_ASSERTIONS = [
  assertApprovalRecordUpgrade,
  assertTransitionReceiptUpgrade,
  assertKnowledgeUpdateProposalUpgrade,
  assertLocalGoPackageImpactPrescanUpgrade,
  assertStrictReviewerScaffold,
  assertGraphSnapshotScaffold,
  assertADRV2Scaffold,
  assertCapabilityRegistryScaffold,
  assertOwnershipProjectionScaffold,
  assertProjectSnapshotScaffold,
  assertContextEngineeringScaffold,
  assertEvidenceClaimScaffold,
  assertPolicyAuthorityScaffold,
  assertADRGovernanceScaffold,
  assertKnowledgeGraphCurationScaffold,
  assertChangeImpactCostRiskScaffold,
  assertWorkIntentScaffold,
  assertAuthenticatedADRApprovalScaffold,
  assertAuthenticatedADRLifecycleScaffold,
  assertAuthenticatedADRLifecycleAuthorityEvidenceScaffold,
  assertLegacyGovernanceReadImportScaffold,
  assertKernelOperationalReferenceScaffold,
  assertKernelDecisionReferenceScaffold,
];

export function assertEngineeringSliceUpgrades(target) {
  for (const verify of SLICE_ASSERTIONS) verify(target);
}

export const LEGACY_ENGINEERING_FILES = [
  join('.agent', 'engineering'),
  ...['completion-evidence', 'backend-decision-package', 'frontend-design-package']
    .map((name) => join('.agent', 'eval', `${name}.schema.yml`)),
  ...[
    'backend-engineering', 'domain-modeling', 'data-modeling-transactions',
    'data-migration-lifecycle', 'api-contract-design', 'distributed-reliability-design',
    'performance-capacity', 'observability-engineering', 'architecture-tradeoff',
    'secure-coding', 'information-interaction-design', 'design-system-accessibility',
    'frontend-client-engineering', 'frontend-code-architecture', 'ui-geometry',
    'evidence-claim-management', 'context-engineering', 'policy-authority',
    'project-snapshot',
  ].map((name) => join('.agent', 'skills', `${name}.md`)),
  join('.arch', 'frontend-architecture.v1.json'),
  join('.arch', 'frontend-architecture-baseline.v1.json'),
  join('.arch', 'frontend-architecture-waivers.v1.json'),
  join('docs', 'design', 'ai-engineering-os'),
  ...[
    '0042-frontend-design-decision-contract.md',
    '0043-frontend-code-architecture-governance.md',
    '0044-business-ui-geometry-contract.md',
    '0045-canonical-evidence-claim-contract.md',
    '0046-local-governance-record-journal.md',
    '0047-shadow-cognitive-atom-projection-v1.md',
    '0048-artifact-provenance-evidence-adapter-v1.md',
    '0049-command-observation-evidence-adapter-v1.md',
    '0050-evolve-repo-locator-evidence-adapter-v1.md',
    '0051-local-gate-command-observation-producer-v1.md',
    '0052-local-evolve-repo-locator-observation-producer-v1.md',
    '0053-local-go-package-dependency-graph-observation-producer-v1.md',
    '0054-local-governance-semantic-view-v1.md',
    '0055-shadow-context-package-v1.md',
    '0056-capability-grant-v1-contract-only.md',
    '0057-authenticated-bootstrap-repo-read-grant-issuance.md',
    '0058-authenticated-bootstrap-repo-read-execution.md',
    '0059-approval-record-v1-contract-only.md',
    '0060-transition-receipt-v1-contract-only.md',
  ].map((name) => join('docs', 'adr', name)),
  ...[
    'governance-evidence-claim-v1.schema.json',
    'governance-record-journal-v1.schema.json',
    'governance-semantic-view-v1.schema.json',
    'context-package-v1.schema.json',
    'capability-grant-v1.schema.json',
    'approval-record-v1.schema.json',
    'transition-receipt-v1.schema.json',
    'bootstrap-grant-issuance-v1.schema.json',
    'bootstrap-repo-read-execution-v1.schema.json',
    'cognitive-atom-projection-v1.schema.json',
    'artifact-evidence-adapter-v1.schema.json',
    'command-observation-evidence-adapter-v1.schema.json',
    'evolve-repo-locator-evidence-adapter-v1.schema.json',
    'local-gate-command-observation-producer-v1.schema.json',
    'local-evolve-repo-locator-observation-producer-v1.schema.json',
    'local-go-package-dependency-graph-observation-producer-v1.schema.json',
  ].map((name) => join('docs', 'contracts', name)),
  ...[
    'governance-evidence-claim-v1.json',
    'governance-semantic-view-v1.json',
    'context-package-v1.json',
    'capability-grant-v1.json',
    'approval-record-v1.json',
    'transition-receipt-v1.json',
    'bootstrap-grant-issuance-v1.json',
    'bootstrap-repo-read-execution-v1.json',
    'cognitive-atom-projection-v1.json',
    'artifact-evidence-adapter-v1.json',
    'command-observation-evidence-adapter-v1.json',
    'evolve-repo-locator-evidence-adapter-v1.json',
    'local-gate-command-observation-producer-v1.json',
    'local-evolve-repo-locator-observation-producer-v1.json',
    'local-go-package-dependency-graph-observation-producer-v1.json',
  ].map((name) => join('docs', 'contracts', 'fixtures', name)),
  ...[
    'agent_engineering_check.py',
    'backend_decision_contract.py', 'governance_engineering_check.py',
    'backend_decision_check.py', 'backend_evidence_check.py', 'backend_package_check.py',
    'frontend_design_check.py', 'completion_evidence_check.py',
    'engineering_check_support.py', 'engineering_detector_check.py', 'workflow_verdict_check.py',
    'test_check_bounded_input.py',
    'engineering_routing_check.py', 'test_agent_engineering_check.py', 'test_reviewer_verdict_check.py',
    'test_backend_decision_check.py', 'test_frontend_design_adversarial.py',
    'test_frontend_business_ui_composition_boundaries.py',
    'test_frontend_business_ui_geometry.py', 'test_frontend_geometry_coordinate_contract.py',
    'test_frontend_design_check.py',
    'frontend_design_test_support.py', 'test_legacy_ai_batch_contract.py',
    'governance_contract_check.py', 'test_governance_contract_check.py',
    'context_package_contract_check.py', 'test_context_package_contract_check.py',
    'capability_grant_contract_check.py', 'test_capability_grant_contract_check.py',
    'test_capability_grant_scope_contract.py',
    'approval_record_contract_check.py', 'test_approval_record_contract_check.py',
    'test_approval_record_capability_grant_contract.py',
    'transition_receipt_contract_check.py', 'test_transition_receipt_contract_check.py',
    'test_transition_receipt_cross_contract.py',
    'test_bootstrap_grant_issuance_contract.py',
    'test_bootstrap_grant_issuance_ledger_contract.py',
    'test_bootstrap_repo_read_execution_contract.py',
    'test_bootstrap_repo_read_execution_ledger_contract.py',
    'cognitive_atom_contract_check.py', 'test_cognitive_atom_contract_check.py',
    'artifact_evidence_adapter_check.py', 'test_artifact_evidence_adapter_check.py',
    'command_observation_evidence_adapter_check.py',
    'test_command_observation_evidence_adapter_check.py',
    'evolve_repo_locator_evidence_adapter_check.py',
    'test_evolve_repo_locator_evidence_adapter_check.py',
    'local_command_observation_producer_check.py',
    'test_local_command_observation_producer_check.py',
    'test_governance_engineering_integration.py',
    'test_governance_evolve_locator_integration.py',
    'test_governance_local_command_observation_producer_integration.py',
  ].map((name) => join('harness', name)),
  ...[
    'governance_contract', 'context_package_contract', 'capability_grant_contract',
    'approval_record_contract', 'transition_receipt_contract',
    'bootstrap_grant_issuance_contract', 'bootstrap_repo_read_execution_contract',
    'engineering_routing', 'agent_engineering', 'cognitive_atom_contract',
    'artifact_evidence_adapter', 'command_observation_evidence_adapter',
    'evolve_repo_locator_evidence_adapter', 'local_command_observation_producer',
    'evolve_locator_observation_producer',
    'go_package_dependency_graph_observation_producer', 'governance_engineering',
  ].map((name) => join('harness', name)),
  ...KNOWLEDGE_UPDATE_PROPOSAL_LEGACY_FILES,
  ...LOCAL_GO_PACKAGE_IMPACT_PRESCAN_LEGACY_FILES,
  ...STRICT_REVIEWER_LEGACY_FILES,
  ...GRAPH_SNAPSHOT_LEGACY_FILES,
  ...ADR_V2_LEGACY_FILES,
  ...CAPABILITY_REGISTRY_LEGACY_FILES,
  ...OWNERSHIP_PROJECTION_LEGACY_FILES,
  ...PROJECT_SNAPSHOT_LEGACY_FILES,
  ...CONTEXT_ENGINEERING_LEGACY_FILES,
  ...EVIDENCE_CLAIM_LEGACY_FILES,
  ...POLICY_AUTHORITY_LEGACY_FILES,
  ...ADR_GOVERNANCE_LEGACY_FILES,
  ...KNOWLEDGE_GRAPH_CURATION_LEGACY_FILES,
  ...CHANGE_IMPACT_COST_RISK_LEGACY_FILES,
  ...WORK_INTENT_LEGACY_FILES,
  ...AUTHENTICATED_ADR_APPROVAL_LEGACY_FILES,
  ...AUTHENTICATED_ADR_LIFECYCLE_LEGACY_FILES,
  ...AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_LEGACY_FILES,
  ...LEGACY_GOVERNANCE_READ_IMPORT_LEGACY_FILES,
  ...KERNEL_OPERATIONAL_REFERENCE_LEGACY_FILES,
  ...KERNEL_DECISION_REFERENCE_LEGACY_FILES,
  ...['check.mjs', 'contract.mjs', 'graph.mjs', 'typescript-adapter.mjs',
    'test_frontend-architecture.mjs']
    .map((name) => join('harness', 'frontend-architecture', name)),
  ...['__init__.py', 'contract.py', 'composition.py', 'composition_support.py',
    'geometry.py', 'governance.py', 'evidence.py', 'model.py', 'package.py']
    .map((name) => join('harness', 'frontend_design', name)),
];
