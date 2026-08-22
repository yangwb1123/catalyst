"""Canonical file and activation references for Agent Engineering v1."""

SPEC_FILES = {
    "activation": "engineering/activation.yml",
    "disciplines": "engineering/disciplines.yml",
    "rules": "engineering/rules.yml",
    "detectors": "engineering/detectors.yml",
    "contexts": "engineering/context-routes.yml",
    "profiles": "engineering/workflow-profiles.yml",
    "completion": "eval/completion-evidence.schema.yml",
    "backend_policy": "engineering/backend-decision-gates.yml",
    "backend_package": "eval/backend-decision-package.schema.yml",
    "frontend_policy": "engineering/frontend-design-gates.yml",
    "frontend_profiles": "engineering/frontend-profiles.yml",
    "frontend_package": "eval/frontend-design-package.schema.yml",
    "frontend_architecture_policy": "engineering/frontend-code-architecture.yml",
    "governance_contracts": "engineering/governance-contracts.yml",
}

PROJECT_REFS = {
    "activation": ".agent/engineering/activation.yml",
    "disciplines": ".agent/engineering/disciplines.yml",
    "rules": ".agent/engineering/rules.yml",
    "detectors": ".agent/engineering/detectors.yml",
    "context_routes": ".agent/engineering/context-routes.yml",
    "workflow_profiles": ".agent/engineering/workflow-profiles.yml",
    "capability_catalog": "docs/design/ai-engineering-os/capability-catalog.v1.yml",
    "capability_skill_map": "docs/design/ai-engineering-os/capability-skill-map.v1.yml",
    "acceptance_policy": ".agent/eval/acceptance.schema.yml",
    "completion_contract": ".agent/eval/completion-evidence.schema.yml",
}

EXTENSION_REFS = {
    "backend_policy": ".agent/engineering/backend-decision-gates.yml",
    "backend_package": ".agent/eval/backend-decision-package.schema.yml",
    "backend_standard": "docs/design/ai-engineering-os/backend-decision-standard.md",
    "frontend_policy": ".agent/engineering/frontend-design-gates.yml",
    "frontend_profiles": ".agent/engineering/frontend-profiles.yml",
    "frontend_package": ".agent/eval/frontend-design-package.schema.yml",
    "frontend_standard": "docs/design/ai-engineering-os/frontend-design-standard.md",
    "frontend_architecture_policy": ".agent/engineering/frontend-code-architecture.yml",
    "frontend_architecture_contract": ".arch/frontend-architecture.v1.json",
    "frontend_architecture_baseline": ".arch/frontend-architecture-baseline.v1.json",
    "frontend_architecture_waivers": ".arch/frontend-architecture-waivers.v1.json",
    "frontend_architecture_standard": "docs/design/ai-engineering-os/frontend-code-architecture-standard.md",
    "governance_contract_registry": ".agent/engineering/governance-contracts.yml",
    "governance_contract_schema": "docs/contracts/governance-evidence-claim-v1.schema.json",
    "governance_journal_schema": "docs/contracts/governance-record-journal-v1.schema.json",
    "governance_semantic_view_schema": "docs/contracts/governance-semantic-view-v1.schema.json",
    "governance_semantic_view_fixture": "docs/contracts/fixtures/governance-semantic-view-v1.json",
    "governance_semantic_view_checker": "harness/governance_engineering/semantic_view.py",
    "context_package_schema": "docs/contracts/context-package-v1.schema.json",
    "context_package_fixture": "docs/contracts/fixtures/context-package-v1.json",
    "context_package_checker": "harness/context_package_contract_check.py",
    "context_package_portable_skill": "skills/context-engineering/SKILL.md",
    "context_package_package_manifest": "skills/context-engineering/references/package-manifest.json",
    "context_engineering_skill_decision": "docs/adr/ADR-0071-portable-context-engineering-skill.md",
    "evidence_claim_portable_skill": "skills/evidence-claim-management/SKILL.md",
    "evidence_claim_package_manifest": "skills/evidence-claim-management/references/package-manifest.json",
    "evidence_claim_validation_skill_decision": "docs/adr/ADR-0072-portable-evidence-claim-validation-skill.md",
    "policy_authority_portable_skill": "skills/policy-authority/SKILL.md",
    "policy_authority_package_manifest": "skills/policy-authority/references/package-manifest.json",
    "policy_authority_portable_decision": "docs/adr/ADR-0073-portable-policy-authority-declaration-assessment-skill.md",
    "adr_governance_portable_skill": "skills/adr-governance/SKILL.md",
    "adr_governance_package_manifest": "skills/adr-governance/references/package-manifest.json",
    "adr_governance_portable_decision": "docs/adr/ADR-0074-portable-adr-governance-proposed-document-validation-skill.md",
    "knowledge_graph_curation_portable_skill": "skills/knowledge-graph-curation/SKILL.md",
    "knowledge_graph_curation_package_manifest": "skills/knowledge-graph-curation/references/package-manifest.json",
    "knowledge_graph_curation_portable_decision": "docs/adr/ADR-0075-portable-knowledge-graph-curation-partial-projectors-skill.md",
    "change_impact_cost_risk_skill": ".agent/skills/change-impact-cost-risk.md",
    "change_impact_cost_risk_portable_skill": "skills/change-impact-cost-risk/SKILL.md",
    "change_impact_cost_risk_package_manifest": "skills/change-impact-cost-risk/references/package-manifest.json",
    "change_impact_cost_risk_portable_decision": "docs/adr/ADR-0076-portable-change-impact-cost-risk-lexical-prescan-skill.md",
    "capability_grant_schema": "docs/contracts/capability-grant-v1.schema.json",
    "capability_grant_fixture": "docs/contracts/fixtures/capability-grant-v1.json",
    "capability_grant_checker": "harness/capability_grant_contract_check.py",
    "approval_record_schema": "docs/contracts/approval-record-v1.schema.json",
    "approval_record_fixture": "docs/contracts/fixtures/approval-record-v1.json",
    "approval_record_checker": "harness/approval_record_contract_check.py",
    "transition_receipt_schema": "docs/contracts/transition-receipt-v1.schema.json",
    "transition_receipt_fixture": "docs/contracts/fixtures/transition-receipt-v1.json",
    "transition_receipt_checker": "harness/transition_receipt_contract_check.py",
    "knowledge_update_proposal_schema": "docs/contracts/knowledge-update-proposal-v1.schema.json",
    "knowledge_update_proposal_fixture": "docs/contracts/fixtures/knowledge-update-proposal-v1.json",
    "knowledge_update_proposal_checker": "harness/knowledge_update_proposal_contract_check.py",
    "bootstrap_grant_issuance_schema": "docs/contracts/bootstrap-grant-issuance-v1.schema.json",
    "bootstrap_grant_issuance_fixture": "docs/contracts/fixtures/bootstrap-grant-issuance-v1.json",
    "bootstrap_grant_issuance_checker": "harness/bootstrap_grant_issuance_contract/check.py",
    "bootstrap_repo_read_execution_schema": "docs/contracts/bootstrap-repo-read-execution-v1.schema.json",
    "bootstrap_repo_read_execution_fixture": "docs/contracts/fixtures/bootstrap-repo-read-execution-v1.json",
    "bootstrap_repo_read_execution_checker": "harness/bootstrap_repo_read_execution_contract/check.py",
    "governance_contract_fixture": "docs/contracts/fixtures/governance-evidence-claim-v1.json",
    "cognitive_atom_schema": "docs/contracts/cognitive-atom-projection-v1.schema.json",
    "cognitive_atom_fixture": "docs/contracts/fixtures/cognitive-atom-projection-v1.json",
    "cognitive_atom_checker": "harness/cognitive_atom_contract_check.py",
    "artifact_evidence_adapter_schema": "docs/contracts/artifact-evidence-adapter-v1.schema.json",
    "artifact_evidence_adapter_fixture": "docs/contracts/fixtures/artifact-evidence-adapter-v1.json",
    "artifact_evidence_adapter_checker": "harness/artifact_evidence_adapter_check.py",
    "command_observation_evidence_adapter_schema": "docs/contracts/command-observation-evidence-adapter-v1.schema.json",
    "command_observation_evidence_adapter_fixture": "docs/contracts/fixtures/command-observation-evidence-adapter-v1.json",
    "command_observation_evidence_adapter_checker": "harness/command_observation_evidence_adapter_check.py",
    "evolve_repo_locator_evidence_adapter_schema": "docs/contracts/evolve-repo-locator-evidence-adapter-v1.schema.json",
    "evolve_repo_locator_evidence_adapter_fixture": "docs/contracts/fixtures/evolve-repo-locator-evidence-adapter-v1.json",
    "evolve_repo_locator_evidence_adapter_checker": "harness/evolve_repo_locator_evidence_adapter_check.py",
    "local_gate_command_observation_producer_schema": "docs/contracts/local-gate-command-observation-producer-v1.schema.json",
    "local_gate_command_observation_producer_fixture": "docs/contracts/fixtures/local-gate-command-observation-producer-v1.json",
    "local_gate_command_observation_producer_checker": "harness/local_command_observation_producer_check.py",
    "local_evolve_repo_locator_observation_producer_schema": "docs/contracts/local-evolve-repo-locator-observation-producer-v1.schema.json",
    "local_evolve_repo_locator_observation_producer_fixture": "docs/contracts/fixtures/local-evolve-repo-locator-observation-producer-v1.json",
    "local_evolve_repo_locator_observation_producer_checker": "harness/evolve_locator_observation_producer/check.py",
    "local_go_package_dependency_graph_observation_producer_schema": "docs/contracts/local-go-package-dependency-graph-observation-producer-v1.schema.json",
    "local_go_package_dependency_graph_observation_producer_fixture": "docs/contracts/fixtures/local-go-package-dependency-graph-observation-producer-v1.json",
    "local_go_package_dependency_graph_observation_producer_checker": "harness/go_package_dependency_graph_observation_producer/check.py",
    "local_go_package_impact_prescan_schema": "docs/contracts/local-go-package-impact-prescan-v1.schema.json",
    "local_go_package_impact_prescan_fixture": "docs/contracts/fixtures/local-go-package-impact-prescan-v1.json",
    "local_go_package_impact_prescan_checker": "harness/local_go_package_impact_prescan_contract_check.py",
    "graph_snapshot_schema": "docs/contracts/graph-snapshot-v1.schema.json",
    "graph_snapshot_fixture": "docs/contracts/fixtures/graph-snapshot-v1.json",
    "graph_snapshot_checker": "harness/graph_snapshot_contract_check.py",
    "graph_snapshot_test_source_schema": "docs/contracts/graph-snapshot-go-test-source-v1.schema.json",
    "graph_snapshot_test_source_fixture": "docs/contracts/fixtures/graph-snapshot-go-test-source-v1.json",
    "graph_snapshot_test_source_checker": "harness/graph_snapshot_contract_check.py",
    "architecture_decision_record_v2_schema": "docs/contracts/architecture-decision-record-v2.schema.json",
    "architecture_decision_record_v2_fixture": "docs/contracts/fixtures/ADR-9001-proposed-boundary.md",
    "architecture_decision_record_v2_checker": "harness/architecture_decision_record_v2_check.py",
    "capability_registry_schema": "docs/contracts/capability-registry-v1.schema.json",
    "capability_registry_fixture": "docs/contracts/fixtures/capability-registry-v1.json",
    "capability_registry_checker": "harness/capability_registry_contract/check.py",
    "planning_capability_ownership_schema": "docs/contracts/planning-capability-ownership-projection-v1.schema.json",
    "planning_capability_ownership_fixture": "docs/contracts/fixtures/planning-capability-ownership-projection-v1.json",
    "planning_capability_ownership_checker": "harness/planning_capability_ownership_projection/check.py",
    "planning_capability_ownership_catalog_source": "docs/design/ai-engineering-os/capability-catalog.v1.yml",
    "planning_capability_ownership_mapping_source": "docs/design/ai-engineering-os/capability-skill-map.v1.yml",
    "project_source_snapshot_schema": "docs/contracts/project-source-snapshot-v1.schema.json",
    "project_source_snapshot_fixture": "docs/contracts/fixtures/project-source-snapshot-v1.json",
    "project_source_snapshot_checker": "harness/project_source_snapshot_contract/check.py",
    "work_intent_v1_schema": "docs/contracts/work-intent-v1.schema.json",
    "work_intent_v1_golden_fixture": "docs/contracts/fixtures/work-intent-v1.json",
    "work_intent_v1_checker": "harness/work_intent_contract_check.py",
    "work_intent_v1_semantic_decision": (
        "docs/adr/ADR-0077-authority-neutral-work-intent-v1-contract.md"
    ),
    "work_intent_v1_candidate_governance_decision": (
        "docs/adr/ADR-0078-work-intent-v1-proposed-candidate-governance-and-"
        "source-distribution.md"
    ),
    "authenticated_adr_approval_v1_schema": (
        "docs/contracts/authenticated-architecture-decision-approval-v1.schema.json"
    ),
    "authenticated_adr_approval_v1_golden_fixture": (
        "docs/contracts/fixtures/authenticated-architecture-decision-approval-v1.json"
    ),
    "authenticated_adr_approval_v1_proposal_fixture": (
        "docs/contracts/fixtures/ADR-9002-authenticated-approval-target.md"
    ),
    "authenticated_adr_approval_v1_checker": (
        "harness/authenticated_adr_approval_contract_check.py"
    ),
    "authenticated_adr_approval_v1_semantic_decision": (
        "docs/adr/ADR-0079-authenticated-architecture-decision-approval-v1-"
        "prerequisite.md"
    ),
    "authenticated_adr_approval_v1_candidate_governance_decision": (
        "docs/adr/ADR-0080-authenticated-architecture-decision-approval-v1-"
        "proposed-candidate-governance-and-source-distribution.md"
    ),
    "authenticated_adr_approval_v1_go_authority_decision": (
        "docs/adr/ADR-0081-authenticated-architecture-decision-approval-"
        "authorization-service-v1.md"
    ),
    "authenticated_adr_lifecycle_v1_schema": (
        "docs/contracts/authenticated-architecture-decision-lifecycle-v1.schema.json"
    ),
    "authenticated_adr_lifecycle_v1_golden_fixture": (
        "docs/contracts/fixtures/authenticated-architecture-decision-lifecycle-v1.json"
    ),
    "authenticated_adr_lifecycle_v1_proposal_head_a_fixture": (
        "docs/contracts/fixtures/ADR-9003-lifecycle-head-a.md"
    ),
    "authenticated_adr_lifecycle_v1_proposal_head_b_fixture": (
        "docs/contracts/fixtures/ADR-9004-lifecycle-head-b.md"
    ),
    "authenticated_adr_lifecycle_v1_proposal_join_fixture": (
        "docs/contracts/fixtures/ADR-9005-lifecycle-join.md"
    ),
    "authenticated_adr_lifecycle_v1_checker": (
        "harness/authenticated_adr_lifecycle_contract_check.py"
    ),
    "authenticated_adr_lifecycle_v1_semantic_decision": (
        "docs/adr/ADR-0082-authenticated-architecture-decision-lifecycle-v1-"
        "prerequisite.md"
    ),
    "authenticated_adr_lifecycle_v1_candidate_governance_decision": (
        "docs/adr/ADR-0083-authenticated-architecture-decision-lifecycle-v1-"
        "proposed-candidate-governance-and-source-distribution.md"
    ),
    "authenticated_adr_lifecycle_v1_go_authority_decision": (
        "docs/adr/ADR-0084-authenticated-architecture-decision-lifecycle-"
        "authority-service-v1.md"
    ),
    "authenticated_adr_lifecycle_v1_go_authority_governance_decision": (
        "docs/adr/ADR-0085-authenticated-architecture-decision-lifecycle-"
        "authority-evidence-and-source-distribution.md"
    ),
    "legacy_governance_read_import_v1_schema": (
        "docs/contracts/legacy-governance-read-import-v1.schema.json"
    ),
    "legacy_governance_read_import_v1_memory_fixture": (
        "docs/contracts/fixtures/legacy-governance-read-import-memory-v1.jsonl"
    ),
    "legacy_governance_read_import_v1_adr_0001_fixture": (
        "docs/contracts/fixtures/legacy-governance-read-import-ADR-0001.md"
    ),
    "legacy_governance_read_import_v1_adr_0002_fixture": (
        "docs/contracts/fixtures/legacy-governance-read-import-ADR-0002.md"
    ),
    "legacy_governance_read_import_v1_request_fixture": (
        "docs/contracts/fixtures/legacy-governance-read-import-request-v1.json"
    ),
    "legacy_governance_read_import_v1_view_fixture": (
        "docs/contracts/fixtures/legacy-governance-read-import-view-v1.json"
    ),
    "legacy_governance_read_import_v1_checker": (
        "harness/legacy_governance_read_import_contract_check.py"
    ),
    "legacy_governance_read_import_v1_semantic_decision": (
        "docs/adr/ADR-0086-legacy-governance-read-only-import-v1.md"
    ),
    "legacy_governance_read_import_v1_governance_decision": (
        "docs/adr/ADR-0087-legacy-governance-read-import-governance-and-"
        "source-distribution.md"
    ),
    "kernel_operational_reference_core_v1_schema": (
        "docs/contracts/kernel-operational-reference-core-v1.schema.json"
    ),
    "kernel_operational_reference_core_v1_golden_fixture": (
        "docs/contracts/fixtures/kernel-operational-reference-closure-v1.json"
    ),
    "kernel_operational_reference_core_v1_checker": (
        "harness/kernel_operational_contract_check.py"
    ),
    "kernel_operational_reference_core_v1_semantic_decision": (
        "docs/adr/ADR-0088-kernel-operational-reference-core-v1.md"
    ),
    "kernel_operational_reference_core_v1_governance_decision": (
        "docs/adr/ADR-0089-kernel-operational-reference-governance-and-"
        "source-distribution.md"
    ),
    "kernel_decision_reference_core_v1_schema": (
        "docs/contracts/kernel-decision-reference-core-v1.schema.json"
    ),
    "kernel_decision_reference_core_v1_golden_fixture": (
        "docs/contracts/fixtures/kernel-decision-reference-closure-v1.json"
    ),
    "kernel_decision_reference_core_v1_checker": (
        "harness/kernel_decision_contract_check.py"
    ),
    "kernel_decision_reference_core_v1_semantic_decision": (
        "docs/adr/ADR-0090-kernel-decision-reference-core-v1.md"
    ),
    "kernel_decision_reference_core_v1_governance_decision": (
        "docs/adr/ADR-0091-kernel-decision-reference-governance-and-"
        "source-distribution.md"
    ),
    "decision_capsule_structural_replay_core_v1_schema": (
        "docs/contracts/decision-capsule-structural-replay-core-v1.schema.json"
    ),
    "decision_capsule_structural_replay_core_v1_golden_fixture": (
        "docs/contracts/fixtures/decision-capsule-structural-replay-v1.json"
    ),
    "decision_capsule_structural_replay_core_v1_checker": (
        "harness/decision_capsule_contract_check.py"
    ),
    "decision_capsule_structural_replay_core_v1_semantic_decision": (
        "docs/adr/ADR-0092-decision-capsule-structural-replay-core-v1.md"
    ),
    "decision_capsule_structural_replay_core_v1_governance_decision": (
        "docs/adr/ADR-0093-decision-capsule-structural-replay-governance-and-"
        "source-distribution.md"
    ),
    "governance_contract_skill": ".agent/skills/evidence-claim-management.md",
    "context_package_skill": ".agent/skills/context-engineering.md",
    "capability_grant_skill": ".agent/skills/policy-authority.md",
    "approval_record_skill": ".agent/skills/policy-authority.md",
    "transition_receipt_skill": ".agent/skills/policy-authority.md",
    "knowledge_update_proposal_skill": ".agent/skills/evidence-claim-management.md",
    "graph_snapshot_skill": ".agent/skills/knowledge-graph-curation.md",
    "architecture_decision_record_v2_skill": ".agent/skills/adr-governance.md",
    "capability_registry_skill": ".agent/skills/capability-registry.md",
    "planning_capability_ownership_skill": ".agent/skills/capability-ownership-projection.md",
    "project_source_snapshot_skill": ".agent/skills/project-snapshot.md",
    "project_source_snapshot_portable_skill": "skills/project-snapshot/SKILL.md",
    "project_source_snapshot_package_manifest": "skills/project-snapshot/references/package-manifest.json",
    "governance_contract_decision": "docs/adr/0045-canonical-evidence-claim-contract.md",
    "governance_journal_decision": "docs/adr/0046-local-governance-record-journal.md",
    "governance_semantic_view_decision": "docs/adr/0054-local-governance-semantic-view-v1.md",
    "context_package_decision": "docs/adr/0055-shadow-context-package-v1.md",
    "capability_grant_decision": "docs/adr/0056-capability-grant-v1-contract-only.md",
    "approval_record_decision": "docs/adr/0059-approval-record-v1-contract-only.md",
    "transition_receipt_decision": "docs/adr/0060-transition-receipt-v1-contract-only.md",
    "knowledge_update_proposal_decision": "docs/adr/0061-knowledge-update-proposal-v1-contract-only.md",
    "bootstrap_grant_issuance_decision": "docs/adr/0057-authenticated-bootstrap-repo-read-grant-issuance.md",
    "bootstrap_repo_read_execution_decision": "docs/adr/0058-authenticated-bootstrap-repo-read-execution.md",
    "cognitive_atom_decision": "docs/adr/0047-shadow-cognitive-atom-projection-v1.md",
    "artifact_evidence_adapter_decision": "docs/adr/0048-artifact-provenance-evidence-adapter-v1.md",
    "command_observation_evidence_adapter_decision": "docs/adr/0049-command-observation-evidence-adapter-v1.md",
    "evolve_repo_locator_evidence_adapter_decision": "docs/adr/0050-evolve-repo-locator-evidence-adapter-v1.md",
    "local_gate_command_observation_producer_decision": "docs/adr/0051-local-gate-command-observation-producer-v1.md",
    "local_evolve_repo_locator_observation_producer_decision": "docs/adr/0052-local-evolve-repo-locator-observation-producer-v1.md",
    "local_go_package_dependency_graph_observation_producer_decision": "docs/adr/0053-local-go-package-dependency-graph-observation-producer-v1.md",
    "local_go_package_impact_prescan_decision": "docs/adr/0062-local-go-package-impact-prescan-v1.md",
    "graph_snapshot_decision": "docs/adr/0065-authority-free-graph-snapshot-v1-contract.md",
    "graph_snapshot_test_source_decision": "docs/adr/0066-local-go-lexical-test-source-graph-snapshot.md",
    "architecture_decision_record_v2_decision": "docs/adr/0067-proposed-only-adr-v2-frontmatter.md",
    "capability_registry_decision": "docs/adr/ADR-0068-authority-neutral-capability-registry-v1.md",
    "planning_capability_ownership_decision": "docs/adr/ADR-0069-planning-capability-ownership-projection-v1.md",
    "project_source_snapshot_decision": "docs/adr/ADR-0070-local-project-source-snapshot-v1.md",
    "governance_contract_standard": "docs/design/ai-engineering-os/governance-contracts.md",
}
